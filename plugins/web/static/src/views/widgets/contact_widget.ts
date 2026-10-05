import { load, props, resource } from "trame";
import type { Values } from "@web/core/orm";
import { Many2OneWidget } from "./many2one_widget";
import { widgetProps, widgets } from "./widget";

/**
 * A many2one to a contact, with what the contact chosen says shown under it: its address unless
 * the field's element names other fields of it, in order — `show="address,email,phone"`.
 */
export class ContactWidget extends Many2OneWidget {
    static override template = "web.ContactWidget";

    override props = props({ ...widgetProps });

    /** The fields of the contact shown under it. */
    get shown(): string[] {
        const show = typeof this.props.attrs.show === "string" ? this.props.attrs.show : "address";
        return show
            .split(",")
            .map((name) => name.trim())
            .filter(Boolean);
    }

    /** The contact chosen, read for what is shown of it; nothing when the user may not read it. */
    @resource accessor details: Values | null = load(
        () => ({ model: this.relation, id: this.id, fields: this.shown }),
        ({ model, id, fields }) =>
            id === null || fields.length === 0
                ? Promise.resolve(null)
                : this.orm
                      .read(model, [id], fields, { names: true })
                      .then((rows) => rows[0] ?? null)
                      .catch(() => null),
    );

    /** What is shown of the contact, a line per field filled in; a record by its name. */
    get lines(): string[] {
        const details = this.details;
        if (!details) {
            return [];
        }
        return this.shown.flatMap((name) => {
            const value = details[name];
            if (value === null || value === undefined || value === "" || value === false) {
                return [];
            }
            return [Array.isArray(value) ? String(value[1] ?? "") : String(value)];
        });
    }
}

widgets.add("contact", ContactWidget);
