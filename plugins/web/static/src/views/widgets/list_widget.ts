import { type ComponentClass, computed, inject, load, props, resource } from "trame";
import { type Fields, Models } from "@web/core/models";
import { Orm, type Values } from "@web/core/orm";
import { Views } from "@web/core/views";
import { type Column, columnOf, widgetFor } from "@web/views/view";
import { RecordSearch } from "./record_search";
import { widgetProps, widgets } from "./widget";
import { X2ManyWidget } from "./x2many_widget";

/**
 * The records of a one2many or a many2many as rows, with the columns of their model's list view.
 *
 * Choosing a row opens its record. Where a view edits it, a row is removed with its cross, and
 * a record added by searching.
 */
export class ListWidget extends X2ManyWidget {
    static override template = "web.ListWidget";
    static components = { RecordSearch };

    override props = props({ ...widgetProps });

    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;
    @inject(Views) views!: Views;

    get model(): string {
        return this.props.field.relation ?? "";
    }

    @resource accessor fields: Fields = load(
        () => this.model,
        (model) => this.models.fields(model),
    );

    @resource accessor arch: string = load(
        () => this.model,
        (model) => this.views.arch(model, "list"),
    );

    @computed get columns(): Column[] {
        const fields = this.fields;
        if (fields === undefined || this.arch === undefined) {
            return [];
        }
        const root = new DOMParser().parseFromString(this.arch, "text/xml").documentElement;
        return Array.from(root.getElementsByTagName("field"), (element) => columnOf(element, fields, this.model));
    }

    /** The rows as read, of every record held; one added is read when it is. */
    @resource accessor read: Values[] = load(
        () => ({ model: this.model, ids: this.ids, fields: this.columns.map((column) => column.name) }),
        ({ model, ids, fields }) =>
            ids.length === 0 || fields.length === 0 ? Promise.resolve([]) : this.orm.read(model, ids, fields, { names: true }),
    );

    /** The records held, in their order; one not read yet by its name alone. */
    get rows(): Values[] {
        const read = new Map((this.read ?? []).map((row) => [row.id as number, row]));
        return this.linked.map(([id, name]) => read.get(id) ?? { id, name });
    }

    override get text(): string {
        return this.linked.length === 1 ? "1 record" : `${this.linked.length} records`;
    }

    cellWidget(column: Column): ComponentClass {
        return column.field.type === "refs" ? widgetFor(column, "tags") : widgetFor(column);
    }

    cellClass(column: Column): string {
        return `o_field_${column.field.type}`;
    }
}

widgets.add("list", ListWidget);
