//! Working out what a form shows before it is saved: its record rebuilt in the cache under a
//! virtual id, the values the user changed applied to it, and the fields computed from them
//! computed again. Nothing reaches the database: no record is created or written, and a virtual
//! record is never saved.
use super::*;
use crate::access::Operation;
use erp_internal_types::FinalInternalField;
use erp_types::field::Command;

/// The first virtual id. Real ids are handed out from 1, so a table never reaches it; and it is
/// still an `INTEGER` to PostgreSQL, so a search naming one simply finds nothing.
pub const VIRTUAL_IDS_FROM: u32 = 2_000_000_000;

/// Whether an id is that of a virtual record, which only the cache holds.
pub fn is_virtual(id: u32) -> bool {
    id >= VIRTUAL_IDS_FROM
}

/// What changing a form's values changes in what it shows.
#[derive(Debug, Default)]
pub struct Onchange {
    /// The record's fields computed again, by name.
    pub values: MapOfFields,
    /// The record's fields that could not be computed, with why.
    pub errors: Vec<(String, String)>,
    /// The lines of its one2many and many2many sent along, computed again.
    pub lines: Vec<OnchangeLines>,
}

/// The lines of one one2many or many2many whose fields were computed again.
#[derive(Debug, Default)]
pub struct OnchangeLines {
    pub field: String,
    pub model: String,
    /// Lines that exist, by id.
    pub updated: Vec<(u32, MapOfFields)>,
    /// Lines being created, by the draft number the caller gave them.
    pub created: Vec<(u32, MapOfFields)>,
    /// Fields of lines that could not be computed: which line, which field, why.
    pub errors: Vec<(LineKey, String, String)>,
}

/// A line of a form: one that exists, by id, or one being created, by its draft number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineKey {
    Id(u32),
    Draft(u32),
}

/// A field of a record: its model, the record, the field.
type FieldOf = (String, u32, String);
/// Fields that could not be computed: why.
type Failed = HashMap<FieldOf, String>;

/// The virtual lines of one field, and what they stand for.
#[derive(Default)]
struct Tracked {
    model: String,
    /// Copies of lines that exist, with the id of the line copied.
    copies: Vec<(u32, u32)>,
    /// Lines being created, with their draft number when the caller gave one.
    created: Vec<(u32, Option<u32>)>,
}

/// How many times computing may mark more fields to compute, before giving up on a cycle.
const MAX_ONCHANGE_ROUNDS: usize = 16;

impl<'mm> Environment<'mm> {
    /// What a form would show once these values are applied to its record — `origin`, or a new
    /// one — without saving anything: the fields computed again, of the record and of the lines of
    /// its one2many and many2many sent along.
    ///
    /// Only the fields depending on a value sent are computed again; for a new record or line,
    /// every computed field is. The lines of a one2many sent along are all rebuilt, so that one
    /// depending on the record is computed again even when it was not sent. `drafts` gives, by
    /// field, the draft number of each line created, in the order its commands create them.
    ///
    /// Held to the right to read `origin`, or to create a record of the model: nothing is written.
    pub fn onchange(
        &mut self,
        model_name: &str,
        origin: Option<u32>,
        values: MapOfFields,
        drafts: &HashMap<String, Vec<Option<u32>>>,
    ) -> Result<Onchange> {
        let model = self.model_manager.try_get_model(model_name)?;
        if !self.computes_any(model_name, &values)? {
            return Ok(Onchange::default());
        }
        let record = match origin {
            Some(id) => {
                self.check_access(model_name, Operation::Read, &[id], &[])?;
                self.existing(model_name, vec![id])?;
                self.copy_virtual(model_name, &[id])?[0]
            }
            None => {
                self.check_model_access(model_name, Operation::Create)?;
                self.new_virtual(model_name)?
            }
        };

        let mut tracked: Vec<(String, Tracked)> = Vec::new();
        for field_name in values.fields.keys() {
            let field = model.try_get_internal_field(field_name)?;
            if let Some(target) = x2many_target(field) {
                tracked.push((
                    field_name.clone(),
                    Tracked {
                        model: target.to_string(),
                        ..Tracked::default()
                    },
                ));
            }
        }
        if origin.is_some() {
            for (field_name, lines) in &mut tracked {
                let field = model.try_get_internal_field(field_name)?;
                if back_field(field).is_some() {
                    self.copy_lines(model_name, record, field, lines)?;
                }
            }
        }

        for (field_name, value) in values.fields {
            let field = model.try_get_internal_field(&field_name)?;
            match value {
                Some(FieldType::Commands(commands)) => {
                    let lines = tracked
                        .iter_mut()
                        .find(|(name, _)| *name == field_name)
                        .map(|(_, lines)| lines)
                        .ok_or_else(|| format!("{model_name}.{field_name} takes no commands"))?;
                    let drafts = drafts.get(&field_name).map(Vec::as_slice).unwrap_or(&[]);
                    self.command_virtual(model_name, record, field, commands, drafts, lines)?;
                }
                value => self.set_virtual(model_name, record, &field_name, value)?,
            }
        }

        let mut records: Vec<(&str, u32)> = vec![(model_name, record)];
        for (_, lines) in &tracked {
            let ids = lines
                .copies
                .iter()
                .map(|(copy, _)| *copy)
                .chain(lines.created.iter().map(|(line, _)| *line));
            records.extend(ids.map(|id| (lines.model.as_str(), id)));
        }
        let (computed, failed) = self.compute_virtual(&records)?;
        let failures_of = |model: &str, id: u32| -> Vec<(String, String)> {
            let mut found: Vec<(String, String)> = failed
                .iter()
                .filter(|((failed_model, failed_id, _), _)| {
                    failed_model == model && *failed_id == id
                })
                .map(|((_, _, field), error)| (field.clone(), error.clone()))
                .collect();
            found.sort();
            found
        };

        let mut onchange = Onchange {
            values: self.read_computed(model_name, record, &computed)?,
            errors: failures_of(model_name, record),
            lines: Vec::new(),
        };
        for (field_name, lines) in tracked {
            let mut result = OnchangeLines {
                field: field_name,
                model: lines.model.clone(),
                ..OnchangeLines::default()
            };
            for (copy, line) in lines.copies {
                for (field, error) in failures_of(&lines.model, copy) {
                    result.errors.push((LineKey::Id(line), field, error));
                }
                let values = self.read_computed(&lines.model, copy, &computed)?;
                if !values.fields.is_empty() {
                    result.updated.push((line, values));
                }
            }
            for (line, draft) in lines.created {
                if let Some(draft) = draft {
                    for (field, error) in failures_of(&lines.model, line) {
                        result.errors.push((LineKey::Draft(draft), field, error));
                    }
                }
                let values = self.read_computed(&lines.model, line, &computed)?;
                if let Some(draft) = draft
                    && !values.fields.is_empty()
                {
                    result.created.push((draft, values));
                }
            }
            onchange.lines.push(result);
        }
        Ok(onchange)
    }

    /// Whether the model, or one whose lines are sent along, computes any field: without one,
    /// nothing a form changes can change what it shows.
    fn computes_any(&self, model_name: &str, values: &MapOfFields) -> Result<bool> {
        let model = self.model_manager.try_get_model(model_name)?;
        let mut models = vec![model];
        for field_name in values.fields.keys() {
            if let Some(target) = x2many_target(model.try_get_internal_field(field_name)?) {
                models.push(self.model_manager.try_get_model(target)?);
            }
        }
        Ok(models
            .iter()
            .any(|model| model.fields.values().any(|field| field.compute.is_some())))
    }

    /// A new virtual record of a model, holding its fields' defaults.
    fn new_virtual(&mut self, model_name: &str) -> Result<u32> {
        let mut values = MapOfFields::default();
        self.fill_default_values_on_map(model_name, &mut values);
        let id = self.virtual_from(model_name, values)?;
        let model = self.model_manager.try_get_model(model_name)?;
        let computed: Vec<&str> = model
            .fields
            .values()
            .filter(|field| field.compute.is_some())
            .map(|field| field.name.as_str())
            .collect();
        self.cache
            .add_ids_to_recompute(model_name, &computed, &[id]);
        Ok(id)
    }

    /// Virtual copies of records that exist, as the database holds them.
    fn copy_virtual(&mut self, model_name: &str, ids: &[u32]) -> Result<Vec<u32>> {
        let model = self.model_manager.try_get_model(model_name)?;
        let fields: Vec<&str> = model
            .fields
            .values()
            .filter(|field| field.is_stored() || x2many_target(field).is_some())
            .filter(|field| field.is_stored() || field.compute.is_none())
            .map(|field| field.name.as_str())
            .collect();
        let rows = self.read_unchecked(model_name, &MultipleIds::from(ids.to_vec()), &fields)?;
        rows.into_iter()
            .map(|row| self.virtual_from(model_name, row))
            .collect()
    }

    /// A virtual record holding these values, and nothing for the fields they leave out: every
    /// field it keeps is in the cache, so nothing is ever looked up for it in the database.
    fn virtual_from(&mut self, model_name: &str, mut values: MapOfFields) -> Result<u32> {
        let id = VIRTUAL_IDS_FROM + self.virtual_count;
        self.virtual_count += 1;
        let model = self.model_manager.try_get_model(model_name)?;
        for field in model.fields.values() {
            if !field.is_stored() && x2many_target(field).is_none() {
                continue;
            }
            let value = match values.fields.remove(&field.name).flatten() {
                Some(value) => Some(value),
                None if x2many_target(field).is_some() => Some(FieldType::Refs(Vec::new())),
                None => None,
            };
            self.cache.insert_field_in_cache(
                model_name,
                &field.name,
                &[id],
                value,
                &Dirty::NotUpdateDirty,
                &Update::UpdateIfExists,
            );
        }
        Ok(id)
    }

    /// Change a field of a virtual record, marking what depends on it to be computed again —
    /// without any of what writing a record does: no check, no stamp, nothing to save.
    fn set_virtual(
        &mut self,
        model_name: &str,
        id: u32,
        field_name: &str,
        value: Option<FieldType>,
    ) -> Result<()> {
        if field_name == "id" {
            return Ok(());
        }
        let value = Self::without_empty_references(value);
        self.cache.insert_field_in_cache(
            model_name,
            field_name,
            &[id],
            value,
            &Dirty::NotUpdateDirty,
            &Update::UpdateIfExists,
        );
        self.check_compute_on_field(model_name, field_name, &[id])
    }

    /// Replace the lines of a virtual record's one2many by virtual copies pointing back to it.
    fn copy_lines(
        &mut self,
        model_name: &str,
        record: u32,
        field: &FinalInternalField,
        lines: &mut Tracked,
    ) -> Result<()> {
        let Some(back) = back_field(field) else {
            return Ok(());
        };
        let held = self.held_by(model_name, record, &field.name);
        let copies = self.copy_virtual(&lines.model, &held)?;
        for copy in &copies {
            self.cache.insert_field_in_cache(
                &lines.model,
                back,
                &[*copy],
                Some(FieldType::Ref(record)),
                &Dirty::NotUpdateDirty,
                &Update::UpdateIfExists,
            );
        }
        self.cache.insert_field_in_cache(
            model_name,
            &field.name,
            &[record],
            Some(FieldType::Refs(copies.clone())),
            &Dirty::NotUpdateDirty,
            &Update::UpdateIfExists,
        );
        lines.copies.extend(copies.into_iter().zip(held));
        Ok(())
    }

    /// Carry out the commands of a virtual record's one2many or many2many on its virtual lines:
    /// changed and created lines are virtual too, and nothing is deleted.
    fn command_virtual(
        &mut self,
        model_name: &str,
        record: u32,
        field: &FinalInternalField,
        commands: Vec<Command>,
        drafts: &[Option<u32>],
        lines: &mut Tracked,
    ) -> Result<()> {
        let target = lines.model.clone();
        let back = back_field(field);
        let mut held = self.held_by(model_name, record, &field.name);
        let mut drafts = drafts.iter().copied();
        let origin_of = |lines: &Tracked, id: u32| {
            lines
                .copies
                .iter()
                .find(|(copy, _)| *copy == id)
                .map_or(id, |(_, line)| *line)
        };
        for command in commands {
            match command {
                Command::Clear => held.clear(),
                Command::Unlink(removed) | Command::Delete(removed) => {
                    held.retain(|id| !removed.contains(&origin_of(lines, *id)));
                }
                Command::Update(records) => {
                    for (id, values) in records {
                        let line = match held.iter().find(|held| origin_of(lines, **held) == id) {
                            Some(line) if is_virtual(*line) => *line,
                            _ => {
                                let copy = self.copy_virtual(&target, &[id])?[0];
                                lines.copies.push((copy, id));
                                held.retain(|held| *held != id);
                                held.push(copy);
                                copy
                            }
                        };
                        for (name, value) in values.fields {
                            self.set_virtual(&target, line, &name, value)?;
                        }
                    }
                }
                Command::Create(records) => {
                    for values in records {
                        let line = self.new_virtual(&target)?;
                        if let Some(back) = back {
                            self.set_virtual(&target, line, back, Some(FieldType::Ref(record)))?;
                        }
                        for (name, value) in values.fields {
                            self.set_virtual(&target, line, &name, value)?;
                        }
                        lines.created.push((line, drafts.next().flatten()));
                        held.push(line);
                    }
                }
                Command::Link(linked) => {
                    for id in linked {
                        if !held.contains(&id) {
                            held.push(id);
                        }
                    }
                }
            }
        }
        self.set_virtual(model_name, record, &field.name, Some(FieldType::Refs(held)))
    }

    /// Compute every field marked to be computed on these virtual records, until computing marks
    /// no more; the fields computed, and those that could not be, by record.
    ///
    /// A field whose computation fails is told apart by computing the others one by one: they are
    /// still computed, and what failed is logged, since it is mostly a mistake of the method's.
    fn compute_virtual(&mut self, records: &[(&str, u32)]) -> Result<(HashSet<FieldOf>, Failed)> {
        let mut computed = HashSet::new();
        let mut failed = Failed::new();
        for _ in 0..MAX_ONCHANGE_ROUNDS {
            let mut pending: Vec<(&str, u32, Vec<&'mm str>)> = Vec::new();
            for (model_name, id) in records {
                let model = self.model_manager.try_get_model(model_name)?;
                let marked: Vec<&'mm str> = model
                    .fields
                    .values()
                    .filter(|field| field.compute.is_some())
                    .filter(|field| {
                        self.cache
                            .is_field_to_recompute(model_name, &field.name, *id)
                    })
                    .map(|field| field.name.as_str())
                    .collect();
                if !marked.is_empty() {
                    pending.push((model_name, *id, marked));
                }
            }
            if pending.is_empty() {
                return Ok((computed, failed));
            }
            for (model_name, id, fields) in pending {
                let record = SingleId::from(id);
                if self.read_unchecked(model_name, &record, &fields).is_ok() {
                    for field in fields {
                        computed.insert((model_name.to_string(), id, field.to_string()));
                    }
                    continue;
                }
                for field in fields {
                    match self.read_unchecked(model_name, &record, &[field]) {
                        Ok(_) => {
                            computed.insert((model_name.to_string(), id, field.to_string()));
                        }
                        Err(error) => {
                            tracing::warn!(
                                model = model_name,
                                field,
                                %error,
                                "A field of a form could not be computed"
                            );
                            self.cache
                                .remove_ids_from_recompute(model_name, &[field], &[id]);
                            failed.insert(
                                (model_name.to_string(), id, field.to_string()),
                                error.to_string(),
                            );
                        }
                    }
                }
            }
        }
        Err("Computing the form's fields does not settle: they depend on each other".into())
    }

    /// The fields of a virtual record that were computed, with their values.
    fn read_computed(
        &mut self,
        model_name: &str,
        id: u32,
        computed: &HashSet<FieldOf>,
    ) -> Result<MapOfFields> {
        let fields: Vec<&str> = computed
            .iter()
            .filter(|(model, record, _)| model == model_name && *record == id)
            .map(|(_, _, field)| field.as_str())
            .collect();
        if fields.is_empty() {
            return Ok(MapOfFields::default());
        }
        let mut rows = self.read_unchecked(model_name, &SingleId::from(id), &fields)?;
        let mut values = rows.pop().unwrap_or_default();
        values.fields.remove("id");
        Ok(values)
    }

    /// The records a virtual record's one2many or many2many holds, as the cache has them.
    fn held_by(&self, model_name: &str, id: u32, field_name: &str) -> Vec<u32> {
        match self.cache.get_field_from_cache(model_name, field_name, id) {
            Some(FieldType::Refs(ids)) => ids.clone(),
            _ => Vec::new(),
        }
    }

    /// The records of a model whose many2one points to one of these virtual records, as the cache
    /// holds them: no database ever does.
    pub(super) fn virtual_pointing_to(
        &self,
        model_name: &str,
        field_name: &str,
        targets: &[u32],
    ) -> Vec<u32> {
        let Some(models) = self.cache.cache.get(model_name) else {
            return Vec::new();
        };
        models
            .models
            .iter()
            .filter(|(_, record)| {
                record
                    .get_field(field_name)
                    .and_then(|field| field.get())
                    .is_some_and(
                        |value| matches!(value, FieldType::Ref(target) if targets.contains(target)),
                    )
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

/// The model a one2many or a many2many holds records of.
fn x2many_target(field: &FinalInternalField) -> Option<&'static str> {
    match &field.inverse {
        Some(FieldReference {
            target_model,
            inverse_field: FieldReferenceType::O2M { .. } | FieldReferenceType::M2M { .. },
        }) => Some(target_model),
        _ => None,
    }
}

/// The many2one by which the lines of a one2many point back to their record.
fn back_field(field: &FinalInternalField) -> Option<&str> {
    match &field.inverse {
        Some(FieldReference {
            inverse_field: FieldReferenceType::O2M { inverse_field },
            ..
        }) => Some(inverse_field.as_str()),
        _ => None,
    }
}
