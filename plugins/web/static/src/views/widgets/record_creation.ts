import type { Models } from "@web/core/models";
import type { Orm, Values } from "@web/core/orm";
import type { Choice } from "./record_search";

/** Create a record from a name alone; `null` when the server refuses, for a form to be opened. */
export async function createByName(orm: Orm, model: string, name: string): Promise<Choice | null> {
    try {
        return await orm.nameCreate(model, name);
    } catch {
        return null;
    }
}

/** What a form creating a record from a name starts with: the name, in the field naming them. */
export async function nameDefaults(models: Models, model: string, name: string): Promise<Values> {
    const fields = await models.fields(model);
    const field = Object.entries(fields).find(([, described]) => described.name_field)?.[0];
    return field === undefined ? {} : { [field]: name };
}
