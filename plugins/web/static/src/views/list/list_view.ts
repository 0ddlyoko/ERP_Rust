import { effect, load, props, resource, state, t } from "trame";
import { listMemory, rememberList } from "@web/core/list_memory";
import type { Values } from "@web/core/orm";
import { View, viewKinds, viewProps } from "@web/views/view";

/**
 * Records of a model as rows, one column per field shown, a page at a time.
 *
 * The rows and how many records there are in all load side by side. Rows can be selected, page by
 * page: what is done with a selection comes later, as does the search, shown but not yet applied.
 *
 * Choosing a row opens its record in a form; once some rows are selected, it selects it instead,
 * the way its check box does. The list is remembered per action — its page, its selection, the
 * records it shows — so coming back from a form finds it as it was, and the form steps through
 * those records.
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

    @state accessor offset = listMemory(this.router.route.action)?.offset ?? 0;
    @state accessor query = "";
    @state accessor selected = new Set<number>(listMemory(this.router.route.action)?.selected ?? []);

    @effect remember(): void {
        rememberList(this.router.route.action, {
            offset: this.offset,
            selected: [...this.selected],
            ids: (this.records ?? []).map((record) => this.idOf(record)),
            total: this.total ?? 0,
        });
    }

    /** Open a form for a record not created yet. */
    create(): void {
        this.router.go({ ...this.router.route, view: "form", id: null });
    }

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

    /** A row chosen: its record opened, or, while rows are selected, selected or not in turn. */
    choose(record: Values): void {
        if (this.selected.size > 0) {
            this.select(record, !this.selected.has(this.idOf(record)));
            return;
        }
        this.router.go({ ...this.router.route, view: "form", id: this.idOf(record) });
    }

    selectAll(checked: boolean): void {
        for (const record of this.records ?? []) {
            this.select(record, checked);
        }
    }
}

viewKinds.add("list", ListView);
