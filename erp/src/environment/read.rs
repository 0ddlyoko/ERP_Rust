//! Reading records: browsing, searching and filling the cache on demand.
use super::*;
use crate::access::{Access, AccessDenied, Operation};
use crate::database::{Group, GroupBy};
use crate::errors::MissingRecords;
use crate::model::CREATE_DATE;
use erp_search::OrderBy;
use erp_types::field::FieldKind;
use std::borrow::Cow;

impl<'mm> Environment<'mm> {
    pub fn get_empty_record<M>(&self) -> M
    where
        M: Model<MultipleIds>,
    {
        M::create_instance(MultipleIds::default())
    }

    /// The names of the records the caller may read, by id: by the field naming the model's
    /// records.
    ///
    /// Missing for one it may not read — a model it may read nothing of included, so that one
    /// relation out of reach does not fail a whole read — and for every record of a model naming
    /// none, or named by a private field: a name is what any reader of the record sees, never
    /// more.
    pub fn names(&mut self, model_name: &str, ids: &[u32]) -> Result<HashMap<u32, String>> {
        let model = self.model_manager.try_get_model(model_name)?;
        let Some(name_field) = model
            .name_field()
            .filter(|field| !model.fields[*field].private)
            .map(str::to_string)
        else {
            return Ok(HashMap::new());
        };
        let ids = ids.to_vec();
        let readable = match self.access(model_name, Operation::Read)? {
            Access::Unrestricted | Access::Restricted(SearchType::Nothing) => {
                self.present(model_name, ids)?.0
            }
            Access::Denied => return Ok(HashMap::new()),
            Access::Restricted(_) => {
                self.search_ids(model_name, &make_domain!([("id", "in", ids)]))?
            }
        };
        let rows = self.read(
            model_name,
            &MultipleIds::from(readable),
            &[name_field.as_str()],
        )?;
        let mut names = HashMap::new();
        for row in rows {
            let id = *row.get::<&u32>("id");
            if let Some(name) = row.get_option::<&String>(&name_field) {
                names.insert(id, name.clone());
            }
        }
        Ok(names)
    }

    /// The records the caller may read whose name holds `text`, whatever its case, by name: at most
    /// `limit` of them, as `(id, name)`.
    ///
    /// What is typed is matched as written: `%` and `_` are characters to find, not wildcards. A
    /// model naming none, named by a private field, or one the caller may read nothing of, has
    /// nothing to find.
    pub fn name_search(
        &mut self,
        model_name: &str,
        text: &str,
        limit: usize,
    ) -> Result<Vec<(u32, String)>> {
        self.name_search_within(model_name, text, &SearchType::Nothing, limit)
    }

    /// Same, among the records matching a domain as well: those a field may point to.
    pub fn name_search_within(
        &mut self,
        model_name: &str,
        text: &str,
        within: &SearchType,
        limit: usize,
    ) -> Result<Vec<(u32, String)>> {
        let model = self.model_manager.try_get_model(model_name)?;
        let Some(name_field) = model
            .name_field()
            .filter(|field| !model.fields[*field].private)
            .map(str::to_string)
        else {
            return Ok(Vec::new());
        };
        let escaped: String = text
            .chars()
            .flat_map(|c| match c {
                '\\' | '%' | '_' => vec!['\\', c],
                c => vec![c],
            })
            .collect();
        let pattern = format!("%{escaped}%");
        let named = SearchType::Tuple(erp_search::SearchTuple {
            left: LeftTuple::from(name_field.as_str()),
            operator: erp_search::SearchOperator::ILike,
            right: erp_search::RightTuple::String(pattern),
        });
        let domain = match within {
            SearchType::Nothing => named,
            within => SearchType::And(Box::new(named), Box::new(within.clone())),
        };
        let options = SearchOptions {
            limit: Some(limit),
            offset: 0,
            order: vec![erp_search::OrderBy::asc(&name_field)],
        };
        let found = match self.search_ids_with(model_name, &domain, &options) {
            Ok(found) => found,
            Err(error) if error.downcast_ref::<AccessDenied>().is_some() => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let rows = self.read(
            model_name,
            &MultipleIds::from(found),
            &[name_field.as_str()],
        )?;
        Ok(rows
            .iter()
            .map(|row| {
                let id = *row.get::<&u32>("id");
                let name = row
                    .get_option::<&String>(&name_field)
                    .cloned()
                    .unwrap_or_default();
                (id, name)
            })
            .collect())
    }

    /// Returns an instance of given model for a specific id
    ///
    /// Do not check if given id is valid id, or is present in the cache
    ///
    /// Do not load given id to the cache
    pub fn get_record<M, Mode: IdMode>(&self, id: Mode) -> M
    where
        M: Model<Mode>,
    {
        M::create_instance(id)
    }

    /// Search given domain for given model, and return an instance of given model if found
    ///
    /// If not found, return an empty recordset
    ///
    /// This method does not load in cache any data related to the model.
    /// It only performs a search, and return the given ids.
    ///
    /// Before performing any search, save any data related to any field given in the domain.
    pub fn search<M>(&mut self, domain: &SearchType) -> Result<M>
    where
        M: Model<MultipleIds>,
    {
        let ids = self.search_ids(M::_get_model_name(), domain)?;
        Ok(M::create_instance(ids.into()))
    }

    /// Search a model addressed by name, and return the matching ids.
    ///
    /// The entry point for callers that only hold a model name at runtime. Like
    /// [`Environment::search`], it first flushes every field the domain mentions.
    pub fn search_ids(&mut self, model_name: &str, domain: &SearchType) -> Result<Vec<u32>> {
        self.search_ids_with(model_name, domain, &SearchOptions::default())
    }

    /// Same as [`Environment::search_ids`], ordered and paginated.
    ///
    /// Only finds what the caller may read: their read rights are added to the domain.
    pub fn search_ids_with(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        options: &SearchOptions,
    ) -> Result<Vec<u32>> {
        let domain = self.readable_domain(model_name, domain)?;
        self.search_ids_unchecked(model_name, &domain, options)
    }

    /// These records, each once and in the order given, refused when one of them does not exist.
    /// Id 0 is no record, and is left out without a word.
    ///
    /// For ids a caller sends: a record named twice is still one record, and one that is not
    /// there — never created, or deleted since — is the caller's mistake, said as such rather
    /// than met halfway through a write. Whether the caller may touch them is not checked here.
    ///
    /// Records known to the cache exist; the others have their stored fields loaded, as reading
    /// them would, and those the database does not return are missing. So the check costs no
    /// query of its own: the one it makes is the one a read or write of them would make.
    pub fn existing(&mut self, model_name: &str, ids: Vec<u32>) -> Result<MultipleIds> {
        let (present, missing) = self.present(model_name, ids)?;
        if !missing.is_empty() {
            return Err(MissingRecords {
                model_name: model_name.to_string(),
                ids: missing,
            }
            .into());
        }
        Ok(MultipleIds::from(present))
    }

    /// These records, each once and in the order given, split into those that exist and those
    /// that do not; id 0 is neither. Checked as [`Environment::existing`] does, through the cache.
    pub(crate) fn present(
        &mut self,
        model_name: &str,
        ids: Vec<u32>,
    ) -> Result<(Vec<u32>, Vec<u32>)> {
        self.model_manager.try_get_model(model_name)?;
        let mut seen = HashSet::with_capacity(ids.len());
        let ids = MultipleIds::from(
            ids.into_iter()
                .filter(|id| *id != 0 && seen.insert(*id))
                .collect::<Vec<u32>>(),
        );
        if ids.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        self.ensure_fields_in_cache(model_name, CREATE_DATE, &ids)?;
        let missing: HashSet<u32> = self
            .cache
            .get_ids_not_in_cache(model_name, CREATE_DATE, ids.get_ids_ref())
            .into_iter()
            .collect();
        Ok(ids.ids.into_iter().partition(|id| !missing.contains(id)))
    }

    /// Same as [`Environment::search_ids_with`], whatever the caller's rights.
    pub(crate) fn search_ids_unchecked(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        options: &SearchOptions,
    ) -> Result<Vec<u32>> {
        let options = self.ordered(model_name, options)?;
        self.prepare_search(model_name, domain, &options)?;
        self.database
            .find_ids(model_name, domain, self.model_manager, &options)
    }

    /// The order the lines of a one2many come in: their model's. A field of it being computed
    /// right now is left out, as Odoo protects it: computing it again to sort would run the very
    /// compute that may be reading these lines.
    fn lines_order(&self, model_name: &str) -> Result<SearchOptions> {
        let mut options = self
            .ordered(model_name, &SearchOptions::default())?
            .into_owned();
        options.order.retain(|order| {
            !self
                .computing
                .iter()
                .any(|(model, field)| model == model_name && *field == order.field)
        });
        Ok(options)
    }

    /// The options a search runs with: those asked, in the model's own order when they name none.
    fn ordered<'o>(
        &self,
        model_name: &str,
        options: &'o SearchOptions,
    ) -> Result<Cow<'o, SearchOptions>> {
        let model = self.model_manager.try_get_model(model_name)?;
        if !options.order.is_empty() || model.order.is_empty() {
            return Ok(Cow::Borrowed(options));
        }
        let mut ordered = options.clone();
        ordered.order = model
            .order
            .iter()
            .map(|(field, descending)| {
                if *descending {
                    OrderBy::desc(field)
                } else {
                    OrderBy::asc(field)
                }
            })
            .collect();
        Ok(Cow::Owned(ordered))
    }

    /// Same, loading the stored fields of the records found in the same query, as reading them
    /// would; what the cache already holds of them is kept.
    fn search_and_load(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        options: &SearchOptions,
    ) -> Result<Vec<u32>> {
        let options = self.ordered(model_name, options)?;
        self.prepare_search(model_name, domain, &options)?;
        let fields = self
            .model_manager
            .try_get_model(model_name)?
            .get_stored_fields();
        let rows =
            self.database
                .search(model_name, &fields, domain, self.model_manager, &options)?;
        let mut ids = Vec::with_capacity(rows.len());
        for (id, values) in rows {
            for (field_name, value) in values {
                self.save_field_to_cache(
                    model_name,
                    field_name,
                    &SingleId::from(id),
                    value.map(FieldType::from),
                    &Dirty::NotUpdateDirty,
                    &Update::NotUpdateIfExists,
                )?;
            }
            ids.push(id);
        }
        Ok(ids)
    }

    /// Check a search before it runs, and save what it reads that is still only in the cache.
    fn prepare_search(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        options: &SearchOptions,
    ) -> Result<()> {
        self.refuse_unstored_in_domain(model_name, domain)?;
        self.save_domain_fields_to_db(model_name, domain)?;
        // Sort keys are checked against the registry before any backend sees them. Identifiers
        // are quoted on the way into SQL, so an unchecked name could not inject anything — but
        // it would reach PostgreSQL as an unknown column and come back as a database error,
        // while the in-memory backend would quietly sort on nothing. Refusing it here makes both
        // behave alike, and gives a caller something it can act on.
        //
        // The primary key is the exception: a real column that no model declares, which both
        // backends can already sort on, and which nothing has to flush because it is never dirty.
        let model = self.model_manager.try_get_model(model_name)?;
        for order in &options.order {
            if order.field == "id" {
                continue;
            }
            model.try_get_internal_field(&order.field)?;
            self.refuse_unstored(model_name, std::slice::from_ref(&order.field), "sort by")?;
        }
        // Ordering reads stored values, so anything still dirty has to reach the database first.
        if options
            .order
            .iter()
            .any(|order| model.is_computed_field(&order.field))
        {
            self.recompute_all_stored()?;
        }
        for order in &options.order {
            if order.field == "id" {
                continue;
            }
            self.save_fields_to_db(model_name, &[order.field.as_str()])?;
        }
        Ok(())
    }

    /// Same as [`Environment::search`], ordered and paginated.
    pub fn search_with<M>(&mut self, domain: &SearchType, options: &SearchOptions) -> Result<M>
    where
        M: Model<MultipleIds>,
    {
        let ids = self.search_ids_with(M::_get_model_name(), domain, options)?;
        Ok(M::create_instance(ids.into()))
    }

    /// Search and read in one call.
    ///
    /// Paging and ordering are applied by the database, so only the records that will be returned
    /// are ever materialised. Values then come back through the cache like [`Environment::read`],
    /// which means computed fields are computed.
    pub fn read_matching(
        &mut self,
        model_name: &str,
        fields: &[&str],
        domain: &SearchType,
        options: &SearchOptions,
    ) -> Result<Vec<MapOfFields>> {
        let domain = self.readable_domain(model_name, domain)?;
        let ids = self.search_and_load(model_name, &domain, options)?;
        self.read_unchecked(model_name, &MultipleIds::from(ids), fields)
    }

    /// Count the records matching a domain.
    ///
    /// Counting happens before any limit would apply, which is why it is not a search option.
    pub fn count(&mut self, model_name: &str, domain: &SearchType) -> Result<u32> {
        let domain = self.readable_domain(model_name, domain)?;
        self.refuse_unstored_in_domain(model_name, &domain)?;
        self.save_domain_fields_to_db(model_name, &domain)?;
        self.database.count(model_name, &domain, self.model_manager)
    }

    /// The records matching a domain, as the caller may read them, gathered as `group_by` says —
    /// by a field's value, or by the period a date falls in — with how many each group holds and
    /// what their `sums` add up to; without `group_by`, all of them in one group.
    ///
    /// What is gathered and summed is kept in a column, and a sum is of numbers: anything else is
    /// refused, as searching on it would be.
    pub fn read_group(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        group_by: Option<&GroupBy>,
        sums: &[&str],
    ) -> Result<Vec<Group>> {
        let model = self.model_manager.try_get_model(model_name)?;
        if let Some(group_by) = group_by {
            let kind = model.try_get_internal_field(&group_by.field)?.kind;
            if kind == FieldKind::Refs {
                return Err(format!(
                    "Field {model_name}.{} holds records: nothing can be grouped by it",
                    group_by.field
                )
                .into());
            }
            if group_by.period.is_some() && !matches!(kind, FieldKind::Date | FieldKind::DateTime) {
                return Err(format!(
                    "Field {model_name}.{} is no date: it has no period to group by",
                    group_by.field
                )
                .into());
            }
            self.refuse_unstored(
                model_name,
                std::slice::from_ref(&group_by.field),
                "group by",
            )?;
        }
        for sum in sums {
            let kind = model.try_get_internal_field(sum)?.kind;
            if !matches!(kind, FieldKind::Integer | FieldKind::Decimal) {
                return Err(format!("Field {model_name}.{sum} is no number: it has no sum").into());
            }
            self.refuse_unstored(model_name, &[sum.to_string()], "sum")?;
        }
        let domain = self.readable_domain(model_name, domain)?;
        self.refuse_unstored_in_domain(model_name, &domain)?;
        self.save_domain_fields_to_db(model_name, &domain)?;
        let mut fields: Vec<&str> = sums.to_vec();
        fields.extend(group_by.map(|group_by| group_by.field.as_str()));
        self.save_fields_to_db(model_name, &fields)?;
        self.database
            .read_group(model_name, &domain, group_by, sums, self.model_manager)
    }

    /// Read fields of records, addressing the model and its fields by name.
    ///
    /// Goes through the cache exactly like the generated accessors, so computed fields are
    /// computed and absent values are loaded. Unlike `dyn Model::get`, a field that is merely
    /// empty comes back as `None` instead of raising `RequiredFieldEmpty`.
    pub fn read<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        fields: &[&str],
    ) -> Result<Vec<MapOfFields>> {
        self.check_access(model_name, Operation::Read, ids.get_ids_ref(), fields)?;
        self.read_unchecked(model_name, ids, fields)
    }

    /// Same, whatever the caller's rights.
    pub(crate) fn read_unchecked<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        fields: &[&str],
    ) -> Result<Vec<MapOfFields>> {
        self.ensure_all_in_cache(model_name, fields, ids)?;
        let mut result: Vec<MapOfFields> = ids
            .get_ids_ref()
            .iter()
            .map(|id| {
                let mut map = MapOfFields::default();
                map.insert_field_type("id", FieldType::Ref(*id));
                map
            })
            .collect();

        for field_name in fields {
            if *field_name == "id" {
                continue;
            }
            // Cloned so the borrow of `self` ends before the next field is read.
            let values: Vec<Option<FieldType>> = self
                .get_fields_value_unchecked(model_name, field_name, ids)?
                .into_iter()
                .map(|value| value.cloned())
                .collect();
            for (index, value) in values.into_iter().enumerate() {
                result[index].insert_option(field_name, value);
            }
        }
        Ok(result)
    }

    /// Refuse a domain naming a field with nothing to compare.
    fn refuse_unstored_in_domain(&self, model_name: &str, domain: &SearchType) -> Result<()> {
        for field in domain.get_fields() {
            self.refuse_unstored(model_name, &field.path, "filter on")?;
        }
        Ok(())
    }

    /// Refuse a path whose every segment is not kept in a column.
    ///
    /// A computed field worked out on each read has none. PostgreSQL answers such a domain with an
    /// unknown column, while the in-memory backend quietly matches nothing and sorts on nothing —
    /// which is worse, because it looks like an answer. Refusing makes both say the same thing,
    /// and say it to whoever asked.
    ///
    /// Logged as well as refused. The caller gets the error, but whoever runs the server is the
    /// one who can fix it — by declaring the field `stored`, or by finding out who keeps asking.
    fn refuse_unstored(&self, model_name: &str, path: &[String], asked: &str) -> Result<()> {
        let mut model = self.model_manager.try_get_model(model_name)?;
        for segment in path {
            // A real column that no model declares, and never computed.
            if segment == "id" {
                break;
            }
            let field = model.try_get_internal_field(segment)?;
            if !field.stored {
                tracing::warn!(
                    model = %model.name,
                    field = %segment,
                    uid = ?self.uid(),
                    "Refused a request to {asked} a field that is worked out on each read"
                );
                let advice = if field.kind.is_stored() {
                    " Declare it `stored` if it has to be."
                } else {
                    ""
                };
                return Err(format!(
                    "Field {}.{segment} is worked out on each read, so nothing can search or sort \
                     on it.{advice}",
                    model.name
                )
                .into());
            }
            match &field.inverse {
                Some(reference) => {
                    model = self.model_manager.try_get_model(reference.target_model)?;
                }
                None => break,
            }
        }
        Ok(())
    }

    /// Get the value of given field for given id.
    ///
    /// If field is not in cache, load it
    ///
    /// If field needs to be computed, compute it
    ///
    /// Refused unless the caller may read the record.
    pub fn get_field_value<'a>(
        &'a mut self,
        model_name: &str,
        field_name: &str,
        id: &SingleId,
    ) -> Result<Option<&'a FieldType>> {
        if id.is_empty() {
            return Ok(None);
        }
        self.check_access(model_name, Operation::Read, &[id.get_id()], &[field_name])?;
        self.ensure_fields_in_cache(model_name, field_name, id)?;
        Ok(self
            .cache
            .get_field_from_cache(model_name, field_name, id.get_id()))
    }

    /// Same as [`Environment::get_field_value`], for several records.
    pub fn get_fields_value<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
    ) -> Result<Vec<Option<&FieldType>>> {
        self.check_access(
            model_name,
            Operation::Read,
            ids.get_ids_ref(),
            &[field_name],
        )?;
        self.get_fields_value_unchecked(model_name, field_name, ids)
    }

    /// Same, whatever the caller's rights.
    pub(crate) fn get_fields_value_unchecked<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
    ) -> Result<Vec<Option<&FieldType>>> {
        self.ensure_fields_in_cache(model_name, field_name, ids)?;
        Ok(ids
            .get_ids_ref()
            .iter()
            .map(|id| self.cache.get_field_from_cache(model_name, field_name, *id))
            .collect())
    }

    /// Add to the ids about to be loaded the others of their recordset that miss this field too.
    ///
    /// Reading a field of one record of a loop then loads it for the whole recordset in one query,
    /// instead of one per record. Capped at [`PREFETCH_MAX`], so a huge recordset is loaded in
    /// batches rather than all at once. Computes are left out: they run as the caller, and working
    /// out records nobody asked for could refuse what the caller never did.
    fn with_prefetch<Mode: IdMode>(
        &self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
        mut to_load: MultipleIds,
    ) -> MultipleIds {
        let room = PREFETCH_MAX.saturating_sub(to_load.ids.len());
        if room == 0 || ids.prefetch_ids().len() <= to_load.ids.len() {
            return to_load;
        }
        let asked: HashSet<u32> = to_load.ids.iter().copied().collect();
        let others: Vec<u32> = self
            .cache
            .get_ids_not_in_cache(model_name, field_name, ids.prefetch_ids())
            .into_iter()
            .filter(|id| !asked.contains(id))
            .take(room)
            .collect();
        to_load.ids.extend(others);
        to_load
    }

    /// Bring several fields of the same records into the cache.
    ///
    /// The computes they need are gathered first — each method on the records that need it — and
    /// run as one plan, so reading several computed fields pays for one savepoint rather than one
    /// per field. What is left is loaded field by field.
    pub(super) fn ensure_all_in_cache<Mode: IdMode>(
        &mut self,
        model_name: &str,
        fields: &[&str],
        ids: &Mode,
    ) -> Result<()> {
        let model = self.model_manager.try_get_model(model_name)?;
        let ids_ref = ids.get_ids_ref();
        let mut plan: Vec<(&'mm str, HashSet<u32>)> = Vec::new();
        for field in fields {
            if *field == "id" {
                continue;
            }
            model.try_get_internal_field(field)?;
            let Some(method) = model.compute_method(field) else {
                continue;
            };
            let mut needed = self.cache.get_ids_to_recompute(model_name, field, ids_ref);
            if !model.is_kept(field) {
                needed.extend(self.cache.get_ids_not_in_cache(model_name, field, ids_ref));
            }
            if needed.is_empty() {
                continue;
            }
            match plan.iter_mut().find(|(planned, _)| *planned == method) {
                Some((_, planned_ids)) => planned_ids.extend(needed),
                None => plan.push((method, needed.into_iter().collect())),
            }
        }
        let plan = plan
            .into_iter()
            .map(|(method, ids)| {
                let mut ids: Vec<u32> = ids.into_iter().collect();
                ids.sort_unstable();
                (method, MultipleIds::from(ids))
            })
            .collect();
        self.call_compute_plan(model_name, plan)?;

        for field in fields {
            if *field != "id" {
                self.ensure_fields_in_cache(model_name, field, ids)?;
            }
        }
        Ok(())
    }

    /// Ensure given field is in cache for given ids
    ///
    /// If some ids are invalid or need to be loaded, load them (or compute them if needed)
    ///
    /// If given field_name is a O2M, load it along with its M2O
    pub(super) fn ensure_fields_in_cache<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
    ) -> Result<()> {
        let ids_ref = ids.get_ids_ref();
        let mut ids_not_in_cache: MultipleIds = self
            .cache
            .get_ids_not_in_cache(model_name, field_name, ids_ref)
            .into();
        let ids_to_recompute: MultipleIds = self
            .cache
            .get_ids_to_recompute(model_name, field_name, ids_ref)
            .into();

        if !ids_not_in_cache.is_empty() || !ids_to_recompute.is_empty() {
            // Load given fields
            let model_info = self.model_manager.try_get_model(model_name)?;
            let field_info = model_info.try_get_internal_field(field_name)?;
            let is_computed_method = field_info.compute.is_some();
            if !ids_to_recompute.is_empty() && is_computed_method {
                self.call_compute_method(model_name, &ids_to_recompute, &[field_name])?;
                // Here, we can assume that ids in ids_to_recompute are in cache
                ids_not_in_cache -= ids_to_recompute;
            }
            if ids_not_in_cache.is_empty() {
                return Ok(());
            }
            if !is_computed_method || model_info.is_stored(field_name) {
                ids_not_in_cache =
                    self.with_prefetch(model_name, field_name, ids, ids_not_in_cache);
            }
            if model_info.is_stored(field_name) {
                // This is a stored field, load it along with all the other stored fields to avoid
                //  multiple database calls
                let fields_to_load = model_info.get_stored_fields();
                self.load_records_fields_from_db(model_name, &ids_not_in_cache, &fields_to_load)?;
            } else if is_computed_method && !model_info.is_kept(field_name) {
                // This could be a computed one. Call it
                self.call_compute_method(model_name, &ids_not_in_cache, &[field_name])?;
            } else if let Some(FieldReference {
                target_model,
                inverse_field:
                    FieldReferenceType::M2M {
                        relation,
                        target_column,
                        ..
                    },
            }) = &field_info.inverse
            {
                // Both sides may hold unwritten pairs, so they reach the relation table before it
                // is read back.
                let target_model = *target_model;
                let relation = relation.clone();
                let target_column = target_column.clone();
                self.save_relations_to_db(model_name, &[field_name])?;
                if let Some(mirror) =
                    self.mirror_of_relation(target_model, &relation, &target_column)
                {
                    self.save_relations_to_db(target_model, &[&mirror])?;
                }

                let model_info = self.model_manager.try_get_model(model_name)?;
                let field_info = model_info.try_get_internal_field(field_name)?;
                let Some(FieldReference {
                    inverse_field:
                        FieldReferenceType::M2M {
                            relation,
                            column,
                            target_column,
                        },
                    ..
                }) = &field_info.inverse
                else {
                    unreachable!("just matched a many2many")
                };
                let pairs = self.database.read_relation(
                    relation,
                    column,
                    target_column,
                    ids_not_in_cache.get_ids_ref(),
                )?;
                for (id, targets) in pairs {
                    self.cache.insert_field_in_cache(
                        model_name,
                        field_name,
                        &[id],
                        Some(FieldType::Refs(targets)),
                        &Dirty::NotUpdateDirty,
                        &Update::UpdateIfExists,
                    );
                }
            } else if let Some(FieldReference {
                target_model,
                inverse_field: FieldReferenceType::O2M { inverse_field },
            }) = &field_info.inverse
            {
                // O2M, save data to the database, and then load the field with related M2O
                // TODO Add search_group(...)
                self.save_fields_to_db(target_model, &[inverse_field])?;
                // Load from database
                let mut result: HashMap<u32, Vec<u32>> =
                    HashMap::with_capacity(ids_not_in_cache.get_ids_ref().len());
                for id in &ids_not_in_cache {
                    result.insert(id.get_id(), vec![]);
                }

                let pointing = self.pointing_within_domain(
                    model_name,
                    field_name,
                    target_model,
                    make_domain!([(inverse_field, "=", ids_not_in_cache)]),
                )?;
                let options = self.lines_order(target_model)?;
                let sorted: Vec<&str> = options
                    .order
                    .iter()
                    .map(|order| order.field.as_str())
                    .filter(|field| *field != "id")
                    .collect();
                self.save_fields_to_db(target_model, &sorted)?;
                let database_result = self.database.search(
                    target_model,
                    &[inverse_field],
                    &pointing,
                    self.model_manager,
                    &options,
                )?;
                for (id, mut map) in database_result {
                    // Data should exist in database, and should not be empty, so we unwrap 2 times
                    let field_value = map.remove(inverse_field.as_str()).unwrap().unwrap();
                    let target_id = match field_value {
                        crate::database::FieldType::UInteger(id) => id,
                        // Only "UInteger" should be there. If it's not the case, there is an issue somewhere
                        _ => panic!("Only UInteger should return here, and not {field_value}"),
                    };
                    result.get_mut(&target_id).unwrap().push(id);
                }

                for (id, ids) in result {
                    // Holding no record is still a list, as a many2many holding none is.
                    let field_value = Some(FieldType::Refs(ids));
                    // Save the O2M to the cache. This will also save the M2O thanks to the save_field_to_cache method
                    self.cache.insert_field_in_cache(
                        model_name,
                        field_name,
                        &[id],
                        field_value,
                        &Dirty::UpdateDirty,
                        &Update::UpdateIfExists,
                    );
                }
            } else {
                return Err(format!(
                    "Field {model_name}.{field_name} is neither kept, computed nor a relation, so \
                     nothing can load it"
                )
                .into());
            }
        }

        Ok(())
    }
}
