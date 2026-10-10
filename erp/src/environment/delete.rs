//! Removing records, and untangling them from their relations on the way out.
use super::*;
use crate::access::Operation;
use crate::model::{DELETE, DeleteArgs};
use erp_types::field::OnDelete;

/// Records of a model pointing through one of its many2one to records being deleted.
struct Pointing<'mm> {
    model: &'mm str,
    field: &'mm str,
    on_delete: OnDelete,
    required: bool,
    automatic: bool,
    ids: Vec<u32>,
}

impl<'mm> Environment<'mm> {
    /// Delete the given records.
    ///
    /// Records of any model pointing to them through a many2one go as that field says: emptied
    /// by default, refusing the deletion when it is `required` or `ondelete = "restrict"`, or
    /// deleted along with them when it is `ondelete = "cascade"`. Their own relational fields are
    /// cleared before the rows go, which pulls them out of the one2many lists of their parents.
    ///
    /// The deletion is written straight to the database, like creation is; rolling the
    /// environment back therefore undoes it. The delete hooks run once the rows are gone.
    ///
    /// Refused as a whole unless the caller may delete every one of the records. What goes with
    /// them through a cascade is not held to the caller's rights: the field asked for it.
    ///
    /// Goes through the model's `delete`, so what a plugin overrode there runs — for records
    /// deleted by a cascade too. The checks of what the records led back to run once done: an
    /// order losing a line is checked again.
    pub fn delete<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<u32> {
        if ids.is_empty() {
            return Ok(0);
        }
        self.model_manager.try_get_model(model_name)?;
        let ids: MultipleIds = ids.clone().into();
        if !self.has_checks(model_name) {
            return self.call_method::<DeleteArgs, u32>(model_name, DELETE, &ids, &());
        }
        self.checked(
            |env| {
                env.note_checks_of_deletion(model_name, &ids)?;
                env.call_method::<DeleteArgs, u32>(model_name, DELETE, &ids, &())
            },
            |_, _| Ok(()),
        )
    }

    /// What deleting does, below every override of `delete`: for records a cascade is deleting,
    /// as part of that deletion and whatever the caller's rights; otherwise as a deletion of its
    /// own.
    pub(crate) fn delete_records(&mut self, model_name: &str, ids: &MultipleIds) -> Result<u32> {
        if ids.is_empty() {
            return Ok(0);
        }
        if let Some(pending) = self.cascading.get_mut(model_name)
            && ids.get_ids_ref().iter().all(|id| pending.contains(id))
        {
            for id in ids.get_ids_ref() {
                pending.remove(id);
            }
            return self.delete_unchecked(model_name, ids.get_ids_ref().clone());
        }
        self.check_access(model_name, Operation::Delete, ids.get_ids_ref(), &[])?;
        // A hook deleting records of its own starts a deletion of its own.
        let outer = std::mem::take(&mut self.deleting);
        let deleted = self
            .delete_unchecked(model_name, ids.get_ids_ref().clone())
            .and_then(|deleted| self.refuse_emptied_relations().map(|()| deleted));
        self.deleting = outer;
        deleted
    }

    /// Same, whatever the caller's rights.
    ///
    /// Records the deletion under way already removes are skipped, so records pointing to each
    /// other through cascades are deleted once, and one pointing to a record deleted alongside
    /// holds nothing back.
    fn delete_unchecked(&mut self, model_name: &str, ids: Vec<u32>) -> Result<u32> {
        let already = self.deleting.entry(model_name.to_string()).or_default();
        let ids: Vec<u32> = ids.into_iter().filter(|id| already.insert(*id)).collect();
        if ids.is_empty() {
            return Ok(0);
        }
        let ids = MultipleIds::from(ids);
        self.forget_access_of(model_name, ids.get_ids_ref())?;
        self.forget_shared_of(model_name);

        // Pending changes must reach the database before the relational fields below start
        // reading through the cache, which can itself trigger a flush.
        self.save_model_to_db(model_name)?;

        let pointing = self.records_pointing_to(model_name, &ids)?;
        Self::refuse_held_back(model_name, &ids, &pointing)?;
        for pointing in pointing {
            match pointing.on_delete {
                OnDelete::Cascade => {
                    let ids = MultipleIds::from(pointing.ids);
                    self.cascading
                        .entry(pointing.model.to_string())
                        .or_default()
                        .extend(ids.get_ids_ref().iter().copied());
                    self.delete(pointing.model, &ids)?;
                }
                OnDelete::SetNull => self.empty_pointing(&pointing)?,
                OnDelete::Restrict => {}
            }
        }

        let model = self.model_manager.try_get_model(model_name)?;
        let relational_fields: Vec<&'mm str> = model
            .fields
            .iter()
            .filter(|(_, field)| !field.automatic)
            .filter_map(|(field_name, field)| field.inverse.as_ref().map(|_| field_name.as_str()))
            .collect();
        for field_name in &relational_fields {
            self.save_field_to_cache(
                model_name,
                field_name,
                &ids,
                None,
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            )?;
        }
        // Emptying a many2one was for the mirrors: written to rows about to go, it would break
        // the NOT NULL of a required one.
        let many2one_fields: Vec<&str> = relational_fields
            .iter()
            .copied()
            .filter(|field_name| {
                matches!(
                    model
                        .fields
                        .get(*field_name)
                        .and_then(|field| field.inverse.as_ref()),
                    Some(FieldReference {
                        inverse_field: FieldReferenceType::M2O { .. },
                        ..
                    })
                )
            })
            .collect();
        self.cache
            .clear_dirty_fields(model_name, &many2one_fields, &ids);
        // Flush the mirrors that were just detached, before the rows disappear. What they may
        // have emptied is judged once the rows are gone, which the rows still there would hide.
        let maybe_emptied = std::mem::take(&mut self.maybe_emptied);
        self.save_all_to_db()?;
        self.maybe_emptied.extend(maybe_emptied);

        let number_of_deletions = self.database.delete(model_name, ids.get_ids_ref())?;
        self.cache.remove_records(model_name, &ids);
        for hook in self.model_manager.delete_hooks.clone() {
            hook(self, model_name, ids.get_ids_ref())?;
        }
        Ok(number_of_deletions)
    }

    /// The records of every model pointing to these through a many2one, those deleted alongside
    /// left out; one entry per field, in the order of the models' and fields' names.
    fn records_pointing_to(
        &mut self,
        model_name: &str,
        ids: &MultipleIds,
    ) -> Result<Vec<Pointing<'mm>>> {
        let model_manager: &'mm ModelManager = self.model_manager;
        let mut fields = Vec::new();
        for model in model_manager.get_models().values() {
            for (field_name, field) in &model.fields {
                if let Some(FieldReference {
                    target_model,
                    inverse_field: FieldReferenceType::M2O { .. },
                }) = &field.inverse
                    && *target_model == model_name
                    && field.is_stored()
                {
                    fields.push((model.name.as_str(), field_name.as_str(), field));
                }
            }
        }
        fields.sort_by_key(|(model, field, _)| (*model, *field));

        let mut pointing = Vec::new();
        for (model, field_name, field) in fields {
            let found: Vec<u32> = self.search_ids_unchecked(
                model,
                &make_domain!([(field_name, "=", ids.get_ids_ref().clone())]),
                &SearchOptions::default(),
            )?;
            let skipped = self.deleting.get(model);
            let found: Vec<u32> = found
                .into_iter()
                .filter(|id| skipped.is_none_or(|skipped| !skipped.contains(id)))
                .collect();
            if !found.is_empty() {
                pointing.push(Pointing {
                    model,
                    field: field_name,
                    on_delete: field.on_delete,
                    required: field.required,
                    automatic: field.automatic,
                    ids: found,
                });
            }
        }
        Ok(pointing)
    }

    /// Refuse the deletion when records point to these through a field that keeps them: one
    /// saying `restrict`, or a required one that would otherwise be emptied.
    ///
    /// Checked for every field before any is acted on, so a refusal leaves nothing half done.
    fn refuse_held_back(
        model_name: &str,
        ids: &MultipleIds,
        pointing: &[Pointing<'mm>],
    ) -> Result<()> {
        let listed = |ids: &[u32]| {
            ids.iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", #")
        };
        for pointing in pointing {
            let reason = match pointing.on_delete {
                OnDelete::Restrict => "keeps what it points to",
                OnDelete::SetNull if pointing.required => "is required",
                OnDelete::SetNull | OnDelete::Cascade => continue,
            };
            return Err(format!(
                "{model_name} #{} cannot be deleted: {} #{} points to it through \"{}\", which {reason}",
                listed(ids.get_ids_ref()),
                pointing.model,
                listed(&pointing.ids),
                pointing.field,
            )
            .into());
        }
        Ok(())
    }

    /// Empty the many2one of records pointing to a record being deleted.
    ///
    /// An automatic field — who created or last changed a record — is filled by the ORM alone,
    /// so it is emptied here without going through the checks a caller's write meets.
    fn empty_pointing(&mut self, pointing: &Pointing<'mm>) -> Result<()> {
        if pointing.automatic {
            self.cache.insert_field_in_cache(
                pointing.model,
                pointing.field,
                &pointing.ids,
                None,
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            );
            return Ok(());
        }
        self.save_field_to_cache(
            pointing.model,
            pointing.field,
            &MultipleIds::from(pointing.ids.clone()),
            None,
            &Dirty::UpdateDirty,
            &Update::UpdateIfExists,
        )
    }
}
