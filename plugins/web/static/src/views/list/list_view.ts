import { type ComponentClass, computed, effect, inject, load, loading, props, refresh, resource, state, t } from "trame";
import { and } from "@web/core/domain";
import { listMemory, rememberList, type Sort } from "@web/core/list_memory";
import { Notifications } from "@web/core/notifications";
import type { Domain, Values } from "@web/core/orm";
import { SearchBar } from "@web/views/search/search_bar";
import { defaultFacets, type Facet, readSearchView, type SearchView, searchDomain } from "@web/views/search/search_model";
import { type Column, View, viewKinds, viewProps, widgetFor } from "@web/views/view";
import { listActions } from "./list_actions";

/** Something the actions menu offers on the selection: a button of the view, or a list action. */
interface ActionItem {
    kind: "method" | "action";
    name: string;
    label: string;
}

/** An action waiting for the user to confirm it. */
interface Confirming {
    message: string;
    item: ActionItem;
}

/**
 * Records of a model as rows, one column per field shown, a page at a time.
 *
 * Searched as the model's search view says, and sorted by the column whose header is clicked.
 * The rows and how many records match load side by side.
 *
 * Choosing a row opens its record in a form; once some rows are selected, it selects it instead,
 * the way its check box does, and the selection can be acted on: by the buttons the list's XML
 * declares, which call a method of the model on the records selected, and by the list actions
 * every list offers ([`listActions`](./list_actions.ts)). The list is remembered per action —
 * its page, selection, search, sort and the records it shows — so coming back from a form finds
 * it as it was, and the form steps through those records.
 */
export class ListView extends View {
    static template = "web.ListView";
    static components = { SearchBar };

    override props = props({
        ...viewProps,
        /** How many rows a page holds. */
        limit: t.number().default(100),
    });

    @inject(Notifications) notifications!: Notifications;

    override get kind(): string {
        return "list";
    }

    @state accessor offset = listMemory(this.router.route.action)?.offset ?? 0;
    @state accessor selected = new Set<number>(listMemory(this.router.route.action)?.selected ?? []);
    /** The search as the user left it; until they touch it, the view's default filters. */
    @state accessor facets: Facet[] | null = listMemory(this.router.route.action)?.facets ?? null;
    @state accessor sort: Sort | null = listMemory(this.router.route.action)?.sort ?? null;
    @state accessor actionsOpen = false;
    @state accessor confirming: Confirming | null = null;
    @state accessor running = false;

    /** Remember the list once its rows are there; reading them sooner would hold the view back. */
    @effect remember(): void {
        if (loading(() => this.records) || loading(() => this.total) || loading(() => this.searchView)) {
            return;
        }
        rememberList(this.router.route.action, {
            offset: this.offset,
            selected: [...this.selected],
            facets: this.currentFacets,
            sort: this.sort,
            ids: (this.records ?? []).map((record) => this.idOf(record)),
            total: this.total ?? 0,
        });
    }

    /** Open a form for a record not created yet. */
    create(): void {
        this.router.go({ ...this.router.route, view: "form", id: null });
    }

    @resource accessor searchArch: string = load(
        () => this.props.resModel,
        (model) => this.views.arch(model, "search"),
    );

    @computed get searchView(): SearchView {
        const arch = this.searchArch;
        const fields = this.fields;
        if (arch === undefined || fields === undefined) {
            return { fields: [], filters: [] };
        }
        return readSearchView(arch, fields);
    }

    get currentFacets(): Facet[] {
        return this.facets ?? defaultFacets(this.searchView);
    }

    /** The records shown: those of the action, as the search narrows them. */
    @computed get domain(): Domain {
        return and([[...this.props.domain], searchDomain(this.searchView, this.currentFacets)]);
    }

    readonly setFacets = (facets: Facet[]): void => {
        this.facets = facets;
        this.turnTo(0);
    };

    @resource accessor records: Values[] = load(
        () => ({
            model: this.props.resModel,
            fields: this.columns.map((column) => column.name),
            domain: this.domain,
            order: this.sort === null ? undefined : [`${this.sort.name} ${this.sort.descending ? "desc" : "asc"}`],
            limit: this.props.limit,
            offset: this.offset,
        }),
        ({ model, fields, domain, order, limit, offset }) =>
            this.orm.searchRead(model, domain, fields, { limit, offset, order, names: true }),
    );

    @resource accessor total: number = load(
        () => ({ model: this.props.resModel, domain: this.domain }),
        ({ model, domain }) => this.orm.count(model, domain),
    );

    /** Whether the list can be sorted by a column: one kept in a column, not a list of records. */
    isSortable(column: Column): boolean {
        return column.field.stored && column.field.type !== "refs";
    }

    /** Sort by a column, smallest first; by the same one again, largest first. */
    sortBy(column: Column): void {
        if (!this.isSortable(column)) {
            return;
        }
        const descending = this.sort?.name === column.name && !this.sort.descending;
        this.sort = { name: column.name, descending };
        this.turnTo(0);
    }

    sortOf(column: Column): "ascending" | "descending" | "none" {
        if (this.sort?.name !== column.name) {
            return "none";
        }
        return this.sort.descending ? "descending" : "ascending";
    }

    /** What the actions menu offers: the list's buttons, then every list action. */
    @computed get actionItems(): ActionItem[] {
        const buttons = Array.from(this.archRoot?.querySelectorAll(":scope > buttons > button") ?? [], (button) => {
            const name = button.getAttribute("name") ?? "";
            return { kind: "method" as const, name, label: button.getAttribute("string") ?? name };
        });
        const actions = listActions
            .getEntries()
            .map(([name, action]) => ({ kind: "action" as const, name, label: action.label }));
        return [...buttons, ...actions];
    }

    /** Act on the selection, once confirmed if the action asks. */
    act(item: ActionItem): void {
        this.actionsOpen = false;
        const action = item.kind === "action" ? listActions.get(item.name) : undefined;
        if (action?.confirm !== undefined) {
            this.confirming = { message: action.confirm(this.selected.size), item };
            return;
        }
        void this.run(item);
    }

    confirm(): void {
        const item = this.confirming?.item;
        this.confirming = null;
        if (item !== undefined) {
            void this.run(item);
        }
    }

    /** Run an action on the selection, then show the records as they now are. */
    async run(item: ActionItem): Promise<void> {
        const ids = [...this.selected];
        this.running = true;
        try {
            if (item.kind === "method") {
                await this.orm.call(this.props.resModel, item.name, ids);
            } else {
                await listActions.get(item.name).run({ orm: this.orm, model: this.props.resModel, ids });
            }
            this.selected.clear();
            refresh(() => this.records);
            refresh(() => this.total);
        } catch (error) {
            this.notifications.add("danger", `${item.label} failed: ${error instanceof Error ? error.message : String(error)}`);
        } finally {
            this.running = false;
        }
    }

    /** A row's records shown as tags, whatever their kind: a list would not fit in a cell. */
    override widgetFor(column: Column): ComponentClass {
        return column.field.type === "refs" ? widgetFor(column, "tags") : widgetFor(column);
    }

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
