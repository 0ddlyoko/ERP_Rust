import { Component, type ComponentClass, type PropsOf, registry, t } from "trame";
import type { Domain, FieldDescription } from "@web/core/orm";

/** The props every widget takes; a widget adds its own to them. */
export const widgetProps = {
    /** The values of the record shown. */
    record: t.object(),
    /** The model of that record, where the view knows it. */
    model: t.string().optional(),
    /** The field of the record the widget shows. */
    name: t.string(),
    /** That field, as the server describes it. */
    field: t.any<FieldDescription>(),
    /** The attributes of the field's element in the view's XML, for the widget to read. */
    attrs: t.object().default({}),
    /** Shown only; editing is for views that edit, such as a form. */
    readonly: t.boolean().default(true),
    /** Called with the new value when the user changes it. */
    onChange: t.func<(value: unknown) => void>().optional(),
    /** What the server computed for the records the field holds, by their key, before saving. */
    computed: t.object().default({}),
    /** Why fields of those records could not be computed: by their key, then by field. */
    computeErrors: t.object().default({}),
};

/**
 * How one value of a record is shown, and edited where a view edits.
 *
 * A widget shows `text` in `web.Widget` unless it brings its own template, or, where a view edits
 * it, an input of `inputType` holding `inputValue`, whose text it `parse`s into a value as it is
 * typed — not once the input is left — so the view knows of a change at once. A widget
 * declares its props as `props = props({ ...widgetProps, ...its own })`, once: the schema is read
 * per class.
 */
export abstract class Widget extends Component {
    static template = "web.Widget";

    declare props: PropsOf<typeof widgetProps>;

    /**
     * The records the field offers to point to: as its `domain` attribute in the view says, else
     * as the field declares, else all of them.
     */
    get domain(): Domain {
        const domain = this.props.attrs.domain;
        if (typeof domain === "string") {
            return JSON.parse(domain) as Domain;
        }
        return [...(this.props.field.domain ?? [])];
    }

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

    /** Whether the user edits the value here: the view edits it, and the widget can. */
    get editable(): boolean {
        return !this.props.readonly && this.props.onChange !== undefined && this.canEdit;
    }

    /** Whether this widget has a way to edit its value yet. */
    get canEdit(): boolean {
        return true;
    }

    get inputType(): string {
        return "text";
    }

    /** The value as the input holds it. */
    get inputValue(): string {
        return this.isEmpty ? "" : String(this.value);
    }

    /** What the input holds, as the value the server reads. */
    parse(text: string): unknown {
        return text;
    }

    commit(text: string): void {
        this.props.onChange?.(this.parse(text));
    }

    /** The class of the widget's element, by its field's type. */
    get className(): string {
        return `o_widget o_widget_${this.props.field.type}`;
    }
}

/** Widgets by name. A plugin adds its own here, or replaces one: every view showing it follows. */
export const widgets = registry.category<ComponentClass>("widgets");

/**
 * The widget a field is shown with when a view names none: by its type. The records of a
 * one2many as a list, those of a many2many as tags; a view may show either with the other.
 */
export function defaultWidget(field: FieldDescription): string {
    if (field.type === "ref") {
        return "many2one";
    }
    if (field.type === "refs") {
        return field.relation_kind === "one2many" ? "list" : "tags";
    }
    return field.type;
}
