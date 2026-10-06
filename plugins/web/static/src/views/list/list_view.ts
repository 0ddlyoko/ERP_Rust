import { type ComponentClass, computed, effect, inject, load, loading, props, refresh, resource, state, t } from "trame";
import { and } from "@web/core/domain";
import { listMemory, rememberList, type Sort } from "@web/core/list_memory";
import { Notifications } from "@web/core/notifications";
import type { Domain, Group, Values } from "@web/core/orm";
import { SearchBar } from "@web/views/search/search_bar";
import { defaultFacets, type Facet, groupByOf, readSearchView, type SearchView, searchDomain } from "@web/views/search/search_model";
import { asksReload, opensRecord, type Column, View, viewKinds, viewProps, widgetFor } from "@web/views/view";
import { type ColumnWidths, columnStyle, dragColumn, tableStyle } from "./column_widths";
import { companionFields } from "@web/views/widgets/decimal_widget";
import { listActions } from "./list_actions";

/** Something the actions menu offers on the selection: a button of the view, or a list action. */
interface ActionItem {
    kind: "method" | "action";
    name: string;
    label: string;
}

/** A line of the table: a group's heading, a record, or the records of a group on their way. */
type Line =
    | { kind: "group"; key: string; group: Group }
    | { kind: "record"; key: string; record: Values }
    | { kind: "loading"; key: string };

/** How many records an opened group shows. */
const GROUP_LIMIT = 80;

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
 * every list offers ([`listActions`](./list_actions.ts)). Gathered by a field — the search says
 * so — the rows are groups, counted and summed, each opened to show its records; a column with
 * a `sum` attribute is summed up under the rows. The list is remembered per action —
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
    /** The columns' widths, once the user sized one. */
    @state accessor widths: ColumnWidths | null = null;
    /** The groups opened, by their domain: their records, or `null` while they load. */
    @state accessor openGroups = new Map<string, Values[] | null>();

    resize(event: MouseEvent, name: string): void {
        dragColumn(event, name, this.widths, (widths) => {
            this.widths = widths;
        });
    }

    columnStyle(column: Column): string {
        return columnStyle(this.widths, column.name);
    }

    get tableStyle(): string {
        return tableStyle(this.widths);
    }

    /** Remember the list once its rows are there; reading them sooner would hold the view back. */
    @effect remember(): void {
        if (
            loading(() => this.records) ||
            loading(() => this.total) ||
            loading(() => this.searchView) ||
            loading(() => this.groups)
        ) {
            return;
        }
        const shown = this.lines.flatMap((line) => (line.kind === "record" ? [this.idOf(line.record)] : []));
        rememberList(this.router.route.action, {
            offset: this.grouping === null ? this.offset : 0,
            selected: [...this.selected],
            facets: this.currentFacets,
            sort: this.sort,
            ids: shown,
            total: this.grouping === null ? (this.total ?? 0) : shown.length,
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
            return { fields: [], filters: [], groupBys: [], groupable: [] };
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
        this.openGroups.clear();
        this.turnTo(0);
    };

    /** How the rows are gathered, as the search says, if they are. */
    @computed get grouping(): { groupBy: string; label: string } | null {
        return groupByOf(this.currentFacets);
    }

    /** The columns summed up, as their `sum` attribute asks: numbers kept in a column. */
    @computed get sumColumns(): Column[] {
        return this.columns.filter(
            (column) =>
                column.attrs.sum !== undefined && column.field.stored && ["integer", "decimal"].includes(column.field.type),
        );
    }

    isSummed(column: Column): boolean {
        return this.sumColumns.includes(column);
    }

    /** The groups the rows are gathered in, with their counts and sums. */
    @resource accessor groups: Group[] | null = load(
        () => ({
            model: this.props.resModel,
            domain: this.domain,
            groupBy: this.grouping?.groupBy ?? null,
            sums: this.sumColumns.map((column) => column.name),
        }),
        ({ model, domain, groupBy, sums }) =>
            groupBy === null ? Promise.resolve(null) : this.orm.readGroup(model, domain, groupBy, sums),
    );

    /** What the summed columns add up to, over every record the search finds. */
    @resource accessor totals: Record<string, string> | null = load(
        () => ({ model: this.props.resModel, domain: this.domain, sums: this.sumColumns.map((column) => column.name) }),
        ({ model, domain, sums }) =>
            sums.length === 0
                ? Promise.resolve(null)
                : this.orm.readGroup(model, domain, null, sums).then(([all]) => all?.sums ?? {}),
    );

    /** The lines of the table: the records, or each group followed by its records once opened. */
    get lines(): Line[] {
        if (this.grouping === null) {
            return (this.records ?? []).map((record) => ({ kind: "record", key: `id:${this.idOf(record)}`, record }));
        }
        return this.sortedGroups.flatMap((group): Line[] => {
            const key = this.groupKey(group);
            const opened = this.openGroups.get(key);
            const records: Line[] =
                opened === undefined
                    ? []
                    : opened === null
                      ? [{ kind: "loading", key: `${key}/loading` }]
                      : opened.map((record) => ({ kind: "record", key: `${key}/${this.idOf(record)}`, record }));
            return [{ kind: "group", key, group }, ...records];
        });
    }

    /** The groups in their value's order — a selection's as it lists its values. */
    get sortedGroups(): Group[] {
        const groups = [...(this.groups ?? [])];
        const field = this.fields?.[(this.grouping?.groupBy ?? "").split(":")[0]];
        if (field?.type !== "selection") {
            return groups;
        }
        const order = (field.values ?? []).map(([key]) => key);
        const rank = (group: Group): number => {
            const at = order.indexOf(group.value as string);
            return at < 0 ? order.length : at;
        };
        return groups.sort((left, right) => rank(left) - rank(right));
    }

    private groupKey(group: Group): string {
        return JSON.stringify(group.domain);
    }

    isOpen(group: Group): boolean {
        return this.openGroups.has(this.groupKey(group));
    }

    /** Open a group, its first records read, or close it. */
    async toggleGroup(group: Group): Promise<void> {
        const key = this.groupKey(group);
        if (this.openGroups.has(key)) {
            this.openGroups.delete(key);
            return;
        }
        this.openGroups.set(key, null);
        const fields = [...this.columns.map((column) => column.name), ...companionFields(this.columns, this.fields ?? {})];
        const order = this.sort === null ? undefined : [`${this.sort.name} ${this.sort.descending ? "desc" : "asc"}`];
        try {
            const records = await this.orm.searchRead(this.props.resModel, and([this.domain, group.domain]), fields, {
                limit: GROUP_LIMIT,
                order,
                names: true,
            });
            if (this.openGroups.has(key)) {
                this.openGroups.set(key, records);
            }
        } catch (error) {
            this.openGroups.delete(key);
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
        }
    }

    /** What a group's heading says: its value as the field shows it, `None` for no value. */
    groupLabel(group: Group): string {
        const [name, period] = (this.grouping?.groupBy ?? "").split(":");
        const field = this.fields?.[name];
        const value = group.value;
        if (value === null || value === undefined || value === false || value === "") {
            return "None";
        }
        if (Array.isArray(value)) {
            return String(value[1] ?? `#${value[0]}`);
        }
        if (field?.type === "selection") {
            return field.values?.find(([key]) => key === value)?.[1] ?? String(value);
        }
        if (field?.type === "bool") {
            return value ? "Yes" : "No";
        }
        if (typeof value === "string" && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
            return periodLabel(value, period ?? "day");
        }
        return String(value);
    }

    /** A sum as its column writes numbers: two decimals for amounts, none for counts. */
    sumText(column: Column, sums: Record<string, string>): string {
        const value = sums[column.name];
        if (value === undefined) {
            return "";
        }
        const digits = column.field.type === "integer" ? 0 : Number(column.attrs.digits ?? 2);
        return Number(value).toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
    }

    @resource accessor records: Values[] = load(
        () => ({
            model: this.props.resModel,
            fields: [...this.columns.map((column) => column.name), ...companionFields(this.columns, this.fields ?? {})],
            domain: this.domain,
            order: this.sort === null ? undefined : [`${this.sort.name} ${this.sort.descending ? "desc" : "asc"}`],
            limit: this.props.limit,
            offset: this.offset,
            grouped: this.grouping !== null,
        }),
        ({ model, fields, domain, order, limit, offset, grouped }) =>
            grouped ? Promise.resolve([]) : this.orm.searchRead(model, domain, fields, { limit, offset, order, names: true }),
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
                const answer = await this.orm.call(this.props.resModel, item.name, ids);
                if (asksReload(answer)) {
                    window.location.reload();
                    return;
                }
                const opened = opensRecord(answer);
                if (opened !== null) {
                    await this.router.go({ action: opened.action, view: "form", id: opened.id });
                    return;
                }
            } else {
                await listActions.get(item.name).run({ orm: this.orm, model: this.props.resModel, ids });
            }
            this.selected.clear();
            this.openGroups.clear();
            refresh(() => this.records);
            refresh(() => this.total);
            refresh(() => this.groups);
            refresh(() => this.totals);
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

/** The period starting on `day` as people name it: `October 2026`, `Q4 2026`, `Week of 5 Oct 2026`. */
function periodLabel(day: string, period: string): string {
    const [year, month, date] = day.split("-").map(Number);
    const start = new Date(year, month - 1, date);
    switch (period) {
        case "year":
            return String(year);
        case "quarter":
            return `Q${Math.floor((month - 1) / 3) + 1} ${year}`;
        case "month":
            return start.toLocaleDateString(undefined, { month: "long", year: "numeric" });
        case "week":
            return `Week of ${start.toLocaleDateString(undefined, { dateStyle: "medium" })}`;
        default:
            return start.toLocaleDateString(undefined, { dateStyle: "medium" });
    }
}

viewKinds.add("list", ListView);
