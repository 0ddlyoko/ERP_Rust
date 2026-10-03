//! Keeping required fields filled: a many2one pointing somewhere, a one2many or a many2many
//! holding at least one record, any other field holding a value.
use super::*;
use erp_internal_types::FinalInternalField;

/// Required one2many and many2many fields that a change may have left empty, by model, field
/// and record: checked once the change is over, since one2many lines are taken out before
/// others are put in.
pub(super) type MaybeEmptied = BTreeSet<(String, String, u32)>;

impl<'mm> Environment<'mm> {
    /// Refuse emptying a required field, unless its records are being deleted.
    ///
    /// A computed field is left to its computation, which may well start empty.
    pub(super) fn refuse_empty_required(
        &self,
        model_name: &str,
        field: &FinalInternalField,
        ids: &[u32],
        value: &Option<FieldType>,
    ) -> Result<()> {
        if !is_empty_value(value) || !field.required || field.compute.is_some() {
            return Ok(());
        }
        if ids.iter().all(|id| self.is_being_deleted(model_name, *id)) {
            return Ok(());
        }
        Err(Self::required_error(model_name, field))
    }

    pub(super) fn required_error(
        model_name: &str,
        field: &FinalInternalField,
    ) -> Box<dyn Error + Send + Sync> {
        format!(
            "Field \"{}\" of model \"{model_name}\" is required: it cannot be left empty",
            field.name
        )
        .into()
    }

    /// Note records whose required one2many or many2many a change may have emptied, to check
    /// once it is over; nothing for a field that is not required.
    pub(super) fn note_maybe_emptied(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: impl IntoIterator<Item = u32>,
    ) -> Result<()> {
        let field = self
            .model_manager
            .try_get_model(model_name)?
            .try_get_internal_field(field_name)?;
        if !field.required || field.compute.is_some() {
            return Ok(());
        }
        for id in ids {
            self.maybe_emptied
                .insert((model_name.to_string(), field_name.to_string(), id));
        }
        Ok(())
    }

    /// Refuse the change when it left a required one2many or many2many empty, on a record still
    /// there and not being deleted.
    pub(super) fn refuse_emptied_relations(&mut self) -> Result<()> {
        if self.maybe_emptied.is_empty() {
            return Ok(());
        }
        let mut grouped: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
        for (model_name, field_name, id) in std::mem::take(&mut self.maybe_emptied) {
            if !self.is_being_deleted(&model_name, id) {
                grouped
                    .entry((model_name, field_name))
                    .or_default()
                    .push(id);
            }
        }
        for ((model_name, field_name), ids) in grouped {
            let still_there = self.search_ids_unchecked(
                &model_name,
                &make_domain!([("id", "=", ids)]),
                &SearchOptions::default(),
            )?;
            if still_there.is_empty() {
                continue;
            }
            let ids = MultipleIds::from(still_there);
            let emptied = self
                .get_fields_value_unchecked(&model_name, &field_name, &ids)?
                .into_iter()
                .any(|value| is_empty_value(&value.cloned()));
            if emptied {
                let field = self
                    .model_manager
                    .try_get_model(&model_name)?
                    .try_get_internal_field(&field_name)?;
                return Err(Self::required_error(&model_name, field));
            }
        }
        Ok(())
    }

    fn is_being_deleted(&self, model_name: &str, id: u32) -> bool {
        self.deleting
            .get(model_name)
            .is_some_and(|deleting| deleting.contains(&id))
    }
}

/// Whether a value holds nothing: no value, or a list of no records.
pub(super) fn is_empty_value(value: &Option<FieldType>) -> bool {
    match value {
        None => true,
        Some(FieldType::Refs(ids)) => ids.is_empty(),
        Some(FieldType::String(text)) => text.is_empty(),
        Some(_) => false,
    }
}
