import { Component, type ComponentClass, computed, inject, load, type PropsOf, resource, t } from "trame";
import { Models, type Fields } from "../core/models";
import { type FieldDescription, Orm } from "../core/orm";
import { StringWidget } from "./widgets/string_widget";
import { defaultWidget, widgets } from "./widgets/widget";

/** The props every view of records takes; a view adds its own to them. */
export const viewProps = {
    /** The model whose records are shown. */
    resModel: t.string(),
    /** The fields shown, in order; every field the user may see when left out. */
    fieldNames: t.array(t.string()).optional(),
    /** Which records: a search domain, all of them when empty. */
    domain: t.array(t.any()).default([]),
};

/** A field a view shows, and the widget it is shown with when not its type's. */
export interface Column {
    name: string;
    field: FieldDescription;
    widget?: string;
}

/**
 * What every view of records shares — list, form, and those to come: its model, the description of
 * the fields it shows, and the widget each is shown with.
 *
 * A view declares its props as `props = props({ ...viewProps, ...its own })`, once: the schema is
 * read per class, so a base class declaring them too would hide what the view adds.
 */
export abstract class View extends Component {
    declare props: PropsOf<typeof viewProps>;

    @inject(Models) models!: Models;
    @inject(Orm) orm!: Orm;

    @resource accessor fields = load(
        () => this.props.resModel,
        (model) => this.models.fields(model),
    );

    /** The fields shown, in order: those named, or every one with `id` first. */
    @computed get columns(): Column[] {
        const fields: Fields | undefined = this.fields;
        if (fields === undefined) {
            return [];
        }
        const names = this.props.fieldNames ?? ["id", ...Object.keys(fields).filter((name) => name !== "id")];
        return names.map((name) => {
            const field = fields[name];
            if (field === undefined) {
                throw new Error(`Model "${this.props.resModel}" shows no field "${name}"`);
            }
            return { name, field };
        });
    }

    /** The widget a column is shown with: the one it names, or its type's, or plain text. */
    widgetFor(column: Column): ComponentClass {
        return widgets.get(column.widget ?? defaultWidget(column.field), StringWidget);
    }

    /** The class of a field's cells: numbers align right, check marks centre. */
    cellClass(column: Column): string {
        return `o_field_${column.field.type}`;
    }
}
