//! Carrying out the commands of a one2many or a many2many: the records it holds to create,
//! change, take out or delete, and the list of what it holds once they are done.
use super::*;
use erp_types::field::Command;

impl<'mm> Environment<'mm> {
    /// Carry out the commands of one record's one2many or many2many, in order, on what it holds,
    /// then write what it holds once they are done; records a `Delete` names are deleted last.
    ///
    /// A record created for a one2many points back to `record` from the start. One taken out of a
    /// one2many is let go, or deleted if the field owns it, as writing the list does. Changing,
    /// creating or deleting a record is held to the rights on its own model.
    pub(super) fn write_commands(
        &mut self,
        model_name: &str,
        field_name: &str,
        record: u32,
        commands: Vec<Command>,
    ) -> Result<()> {
        let model = self.model_manager.try_get_model(model_name)?;
        let field = model.try_get_internal_field(field_name)?;
        let Some(FieldReference {
            target_model,
            inverse_field,
        }) = &field.inverse
        else {
            return Err(format!("{model_name}.{field_name} holds no records to command").into());
        };
        let back = match inverse_field {
            FieldReferenceType::O2M { inverse_field } => Some(inverse_field.as_str()),
            FieldReferenceType::M2M { .. } => None,
            FieldReferenceType::M2O { .. } => {
                return Err(format!(
                    "{model_name}.{field_name} points to one record, which takes no commands"
                )
                .into());
            }
        };
        let mut ids: Vec<u32> = match self
            .retrieve_field_from_cache_or_database(model_name, field_name, &SingleId::from(record))?
            .pop()
        {
            Some((_, Some(FieldType::Refs(ids)))) => ids,
            Some((_, Some(FieldType::Ref(id)))) => vec![id],
            _ => Vec::new(),
        };
        let mut deleted = Vec::new();
        let link = |ids: &mut Vec<u32>, id: u32| {
            if !ids.contains(&id) {
                ids.push(id);
            }
        };
        for command in commands {
            match command {
                Command::Clear => ids.clear(),
                Command::Unlink(unlinked) => ids.retain(|held| !unlinked.contains(held)),
                Command::Delete(removed) => {
                    ids.retain(|held| !removed.contains(held));
                    deleted.extend(removed);
                }
                Command::Update(records) => {
                    for (id, values) in records {
                        let existing = self.existing(target_model, vec![id])?;
                        self.write(target_model, &existing, values)?;
                        link(&mut ids, id);
                    }
                }
                Command::Create(records) => {
                    for mut values in records {
                        if let Some(back) = back {
                            values.insert(back, record);
                        }
                        let created = self.create_records(target_model, vec![values])?;
                        for id in created.get_ids_ref() {
                            link(&mut ids, *id);
                        }
                    }
                }
                Command::Link(linked) => {
                    for id in linked {
                        link(&mut ids, id);
                    }
                }
            }
        }
        self.save_field_to_cache(
            model_name,
            field_name,
            &SingleId::from(record),
            Some(FieldType::Refs(ids)),
            &Dirty::UpdateDirty,
            &Update::UpdateIfExists,
        )?;
        if !deleted.is_empty() {
            // An owned record taken out is already gone; what is left is deleted here.
            let still: Vec<u32> = self.search_ids_unchecked(
                target_model,
                &make_domain!([("id", "=", deleted)]),
                &SearchOptions::default(),
            )?;
            if !still.is_empty() {
                self.delete(target_model, &MultipleIds::from(still))?;
            }
        }
        Ok(())
    }
}
