import type { Fields } from "@web/core/models";
import type { Orm, Values } from "@web/core/orm";

/**
 * The fields whose groups take records — `group_create="stage,project"` on a list or a kanban:
 * grouped by one of them, each group offers to create a record in it, and passes it its value;
 * a kanban also lets a card be dragged from one to another. Only a many2one or a selection the
 * user may change does: a status a workflow moves is left out, never listed.
 */
export function groupCreateFields(root: Element | undefined, fields: Fields | undefined): string[] {
    const listed = (root?.getAttribute("group_create") ?? "")
        .split(",")
        .map((name) => name.trim())
        .filter(Boolean);
    return listed.filter((name) => {
        const field = fields?.[name];
        return field !== undefined && !field.readonly && ["ref", "selection"].includes(field.type);
    });
}

/** The field a record is created from its title alone with: the view's, else the model's name. */
export function titleField(root: Element | undefined, fields: Fields | undefined): string | null {
    const named = root?.getAttribute("quick_create");
    if (named) {
        return named;
    }
    return Object.keys(fields ?? {}).find((name) => fields?.[name].name_field) ?? null;
}

/** Whether a value is none: a group of no value, a field left empty. */
export function isEmptyValue(value: unknown): boolean {
    return value === undefined || value === null || value === "" || value === false;
}

/** The value a group writes to a record: a record's id rather than `[id, name]`. */
export function groupValue(value: unknown): unknown {
    return Array.isArray(value) ? value[0] : value;
}

/**
 * Whether a record of these values would lack a field it needs, once given what a new one starts
 * with: then a form completes it, rather than it being created from its title.
 */
export async function needsForm(orm: Orm, model: string, fields: Fields, values: Values): Promise<boolean> {
    const unset = Object.keys(fields).filter(
        (name) =>
            name !== "id" &&
            fields[name].required &&
            !fields[name].readonly &&
            fields[name].type !== "bool" &&
            isEmptyValue(values[name]),
    );
    if (unset.length === 0) {
        return false;
    }
    const defaults = await orm.defaultGet(model, unset);
    return unset.some((name) => isEmptyValue(defaults[name]));
}
