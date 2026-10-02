import { load, props, resource, state, t } from "trame";
import type { Values } from "../../core/orm";
import { View, viewKinds, viewProps } from "../view";

/**
 * Records of a model as rows, one column per field shown, a page at a time.
 *
 * The rows and how many records there are in all load side by side. Rows can be selected, page by
 * page: what is done with a selection comes later, as does the search, shown but not yet applied.
 */
export class ListView extends View {
    static template = "web.ListView";

    override props = props({
        ...viewProps,
        /** How many rows a page holds. */
        limit: t.number().default(100),
    });

    override get kind(): string {
        return "list";
    }

    @state accessor offset = 0;
    @state accessor query = "";
    @state accessor selected = new Set<number>();

    @resource accessor records: Values[] = load(
        () => ({
            model: this.props.resModel,
            fields: this.columns.map((column) => column.name),
            domain: [...this.props.domain],
            limit: this.props.limit,
            offset: this.offset,
        }),
        ({ model, fields, domain, limit, offset }) =>
            this.orm.searchRead(model, domain, fields, { limit, offset, names: true }),
    );

    @resource accessor total: number = load(
        () => ({ model: this.props.resModel, domain: [...this.props.domain] }),
        ({ model, domain }) => this.orm.count(model, domain),
    );

    idOf(record: Values): number {
        return record.id as number;
    }

    /** The first and last rows shown, counted from 1: `1-100`. */
    get range(): string {
        if (!this.total) {
            return "0";
        }
        return `${this.offset + 1}-${Math.min(this.offset + this.props.limit, this.total)}`;
    }

    get hasPrevious(): boolean {
        return this.offset > 0;
    }

    get hasNext(): boolean {
        return this.offset + this.props.limit < (this.total ?? 0);
    }

    previous(): void {
        this.turnTo(Math.max(0, this.offset - this.props.limit));
    }

    next(): void {
        this.turnTo(this.offset + this.props.limit);
    }

    private turnTo(offset: number): void {
        this.selected.clear();
        this.offset = offset;
    }

    get allSelected(): boolean {
        return (this.records?.length ?? 0) > 0 && this.records.every((record) => this.selected.has(this.idOf(record)));
    }

    select(record: Values, checked: boolean): void {
        if (checked) {
            this.selected.add(this.idOf(record));
        } else {
            this.selected.delete(this.idOf(record));
        }
    }

    selectAll(checked: boolean): void {
        for (const record of this.records ?? []) {
            this.select(record, checked);
        }
    }
}

viewKinds.add("list", ListView);
