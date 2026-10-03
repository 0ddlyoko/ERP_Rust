import { Component, type ComponentClass, computed, effect, inject, load, type PropsOf, registry, resource, t, untrack } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import { type Fields, Models } from "@web/core/models";
import { type FieldDescription, Orm } from "@web/core/orm";
import { Router } from "@web/core/router";
import { Views } from "@web/core/views";
import { StringWidget } from "./widgets/string_widget";
import { defaultWidget, widgets } from "./widgets/widget";

/** The props every view of records takes; a view adds its own to them. */
export const viewProps = {
    /** The model whose records are shown. */
    resModel: t.string(),
    /** Which records: a search domain, all of them when empty. */
    domain: t.array(t.any()).default([]),
    /** The record shown, by a view showing one; a new one when left out. */
    resId: t.number().optional(),
};

/** Views by kind: `list`, `form`. A plugin adds a kind of its own here, or replaces one. */
export const viewKinds = registry.category<ComponentClass>("views");

/** A field a view shows, as its `<field>` element says. */
export interface Column {
    name: string;
    field: FieldDescription;
    /** The `string` of the element, or the field's own label. */
    label: string;
    /** The widget the element names, if not its type's. */
    widget?: string;
    /** Every attribute of the element, for the widget to read. */
    attrs: Record<string, string>;
}

/**
 * What every view of records shares — list, form, and those to come: its model, the XML the
 * server holds for it, the fields that XML shows and the widget each is shown with.
 *
 * A view says its `kind`, and declares its props as `props = props({ ...viewProps, ...its own })`,
 * once: the schema is read per class, so a base class declaring them too would hide what the view
 * adds.
 */
export abstract class View extends Component {
    declare props: PropsOf<typeof viewProps>;

    @inject(Models) models!: Models;
    @inject(Orm) orm!: Orm;
    @inject(Views) views!: Views;
    @inject(Router) router!: Router;
    @inject(Breadcrumb) breadcrumb!: Breadcrumb;

    /** Which view of the model this is: `list`, `form`. */
    abstract get kind(): string;

    /** Shown, the view brings the breadcrumb's trail to where the user now is. */
    @effect followTrail(): void {
        untrack(() => this.breadcrumb.shown(this.router.route, this.kind === "form"));
    }

    @resource accessor fields: Fields = load(
        () => this.props.resModel,
        (model) => this.models.fields(model),
    );

    @resource accessor arch: string = load(
        () => [this.props.resModel, this.kind] as const,
        ([model, kind]) => this.views.arch(model, kind),
    );

    /** The root element of the view's XML: `<list>`, `<form>`. */
    @computed get archRoot(): Element | undefined {
        if (this.arch === undefined) {
            return undefined;
        }
        return new DOMParser().parseFromString(this.arch, "text/xml").documentElement;
    }

    /** Every field the view shows, in the order its XML names them. */
    @computed get columns(): Column[] {
        const fields = this.fields;
        const root = this.archRoot;
        if (fields === undefined || root === undefined) {
            return [];
        }
        return Array.from(root.getElementsByTagName("field"), (element) => this.columnOf(element, fields));
    }

    /** What a `<field>` element shows, with what its attributes say. */
    protected columnOf(element: Element, fields: Fields): Column {
        const name = element.getAttribute("name") ?? "";
        const field = fields[name];
        if (field === undefined) {
            throw new Error(`Model "${this.props.resModel}" shows no field "${name}"`);
        }
        const attrs = Object.fromEntries(Array.from(element.attributes, (attr) => [attr.name, attr.value]));
        return { name, field, label: attrs.string ?? field.label, widget: attrs.widget, attrs };
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
