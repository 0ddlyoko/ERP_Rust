import { type ComponentClass, computed, effect, inject, load, props, resource, state } from "trame";
import { type Fields, Models } from "@web/core/models";
import { Orm, type Values } from "@web/core/orm";
import { Views } from "@web/core/views";
import { type Column, columnOf, widgetFor } from "@web/views/view";
import { RecordSearch } from "./record_search";
import { widgetProps, widgets } from "./widget";
import { X2ManyWidget } from "./x2many_widget";

/** A row as the list shows it: the record held, its values with what was changed here over them. */
interface Row {
    key: string;
    id: number | null;
    values: Values;
}

/**
 * The records of a one2many or a many2many as rows, with the columns of their model's list view
 * — for a one2many, the one pointing back left out.
 *
 * Where a view edits it, a row is edited in place once clicked, and removed with its bin; a
 * one2many adds a line to fill in, a many2many a record found by searching. What is changed is
 * kept until the record holding them is saved. Elsewhere, choosing a row opens its record.
 */
export class ListWidget extends X2ManyWidget {
    static override template = "web.ListWidget";
    static components = { RecordSearch };

    override props = props({ ...widgetProps });

    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;
    @inject(Views) views!: Views;

    /** The row being edited, by its key. */
    @state accessor editing: string | null = null;
    private hadChanges = false;

    /** Leave the row edited once what was changed is saved or discarded. */
    @effect forgetEditingOnceSaved(): void {
        const hasChanges = this.entries.some((entry) => entry.changes !== null);
        if (this.hadChanges && !hasChanges) {
            this.editing = null;
        }
        this.hadChanges = hasChanges;
    }

    get model(): string {
        return this.props.field.relation ?? "";
    }

    get isOne2Many(): boolean {
        return this.props.field.relation_kind === "one2many";
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
        return Array.from(root.getElementsByTagName("field"), (element) => columnOf(element, fields, this.model)).filter(
            (column) => column.name !== this.props.field.inverse,
        );
    }

    /** The rows as read, of every record held that exists; one added is read when it is. */
    @resource accessor read: Values[] = load(
        () => ({ model: this.model, ids: this.ids, fields: this.columns.map((column) => column.name) }),
        ({ model, ids, fields }) =>
            ids.length === 0 || fields.length === 0 ? Promise.resolve([]) : this.orm.read(model, ids, fields, { names: true }),
    );

    /** The records held, in their order, with what was changed here over what was read. */
    get rows(): Row[] {
        const read = new Map((this.read ?? []).map((row) => [row.id as number, row]));
        return this.entries.map((entry) => {
            const base: Values = entry.id === null ? {} : (read.get(entry.id) ?? { id: entry.id, name: entry.name });
            return { key: entry.key, id: entry.id, values: { ...base, ...(entry.changes ?? {}) } };
        });
    }

    override get text(): string {
        return this.entries.length === 1 ? "1 record" : `${this.entries.length} records`;
    }

    cellWidget(column: Column): ComponentClass {
        return column.field.type === "refs" ? widgetFor(column, "tags") : widgetFor(column);
    }

    cellClass(column: Column): string {
        return `o_field_${column.field.type}`;
    }

    /** Whether a cell is edited: its row is, and its field can be. */
    isReadonly(row: Row, column: Column): boolean {
        return !this.editable || this.editing !== row.key || column.field.readonly;
    }

    /** What a cell's widget calls with the value the user gave. */
    changer(row: Row, name: string): (value: unknown) => void {
        return (value) => this.change(row.key, name, value);
    }

    /** A row chosen: edited in place where the list is edited, its record opened otherwise. */
    choose(row: Row): void {
        if (this.editable) {
            this.editing = row.key;
        } else if (row.id !== null) {
            void this.open(row.id);
        }
    }

    /** A new line, its fields' defaults filled in, edited at once. */
    addLine(): void {
        const fields = this.fields ?? {};
        const defaults = Object.fromEntries(
            this.columns
                .map((column) => column.name)
                .filter((name) => fields[name]?.default !== undefined)
                .map((name) => [name, fields[name].default]),
        );
        this.editing = this.addDraft(defaults);
    }
}

widgets.add("list", ListWidget);
