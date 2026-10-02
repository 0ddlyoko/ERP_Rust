import { Component, type ComponentClass, type PropsOf, registry, t } from "trame";
import type { FieldDescription } from "../../core/orm";

/** The props every widget takes; a widget adds its own to them. */
export const widgetProps = {
    /** The values of the record shown. */
    record: t.object(),
    /** The field of the record the widget shows. */
    name: t.string(),
    /** That field, as the server describes it. */
    field: t.any<FieldDescription>(),
    /** Shown only; editing is for views that edit, such as a form. */
    readonly: t.boolean().default(true),
    /** Called with the new value when the user changes it. */
    onChange: t.func<(value: unknown) => void>().optional(),
};

/**
 * How one value of a record is shown, and edited where a view edits.
 *
 * A widget shows `text` in `web.Widget` unless it brings its own template. A widget declares its
 * props as `props = props({ ...widgetProps, ...its own })`, once: the schema is read per class.
 */
export abstract class Widget extends Component {
    static template = "web.Widget";

    declare props: PropsOf<typeof widgetProps>;

    /** The value shown. */
    get value(): unknown {
        return this.props.record[this.props.name];
    }

    get isEmpty(): boolean {
        return this.value === null || this.value === undefined || this.value === "";
    }

    /** The value as the widget writes it. */
    get text(): string {
        return this.isEmpty ? "" : String(this.value);
    }

    /** The class of the widget's element, by its field's type. */
    get className(): string {
        return `o_widget o_widget_${this.props.field.type}`;
    }
}

/** Widgets by name. A plugin adds its own here, or replaces one: every view showing it follows. */
export const widgets = registry.category<ComponentClass>("widgets");

/** The widget a field is shown with when a view names none: by its type. */
export function defaultWidget(field: FieldDescription): string {
    if (field.type === "ref") {
        return "many2one";
    }
    if (field.type === "refs") {
        return "x2many";
    }
    return field.type;
}
