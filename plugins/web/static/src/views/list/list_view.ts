import { type ComponentClass, computed, effect, inject, load, loading, nextTick, props, refresh, resource, state, t } from "trame";
import { and } from "@web/core/domain";
import { decorationNames, decorationOf } from "@web/core/expression";
import { listMemory, rememberList, type Sort } from "@web/core/list_memory";
import { listKey } from "@web/core/router";
import { Notifications } from "@web/core/notifications";
import type { Domain, Group, Values } from "@web/core/orm";
import { FormDialog } from "@web/views/form/form_dialog";
import { groupCreateFields, groupValue, needsForm, titleField } from "@web/views/group_create";
import { favoritesOf, forgetFavorite, saveFavorite } from "@web/views/search/favorites";
import { FilterChips } from "@web/views/search/filter_chips";
import { SearchBar } from "@web/views/search/search_bar";
import {
    defaultFacets,
    type Facet,
    type Favorite,
    groupByOf,
    readSearchView,
    type SearchView,
    searchDomain,
} from "@web/views/search/search_model";
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
    | { kind: "quick"; key: string; group: Group }
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
    static components = { FilterChips, FormDialog, SearchBar };

    override props = props({
        ...viewProps,
        /** How many rows a page holds. */
        limit: t.number().default(100),
    });

    @inject(Notifications) notifications!: Notifications;

    override get kind(): string {
        return "list";
    }

    @state accessor offset = listMemory(listKey(this.router.route))?.offset ?? 0;
    @state accessor selected = new Set<number>(listMemory(listKey(this.router.route))?.selected ?? []);
    /** The search as the user left it; until they touch it, the view's default filters. */
    @state accessor facets: Facet[] | null = listMemory(listKey(this.router.route))?.facets ?? null;
    @state accessor sort: Sort | null = listMemory(listKey(this.router.route))?.sort ?? null;
    @state accessor actionsOpen = false;
    @state accessor confirming: Confirming | null = null;
    @state accessor running = false;
    /** The columns' widths, once the user sized one. */
    @state accessor widths: ColumnWidths | null = null;
    /** The groups opened, by their domain: their records, or `null` while they load. */
    @state accessor openGroups = new Map<string, Values[] | null>();
    /** The group a record is being created in from its title, by its key. */
    @state accessor adding: string | null = null;
    /** What a record whose title is not enough starts with, while a form completes it. */
    @state accessor completing: Values | null = null;
    /** The group that record is created in. */
    private completingGroup: Group | null = null;
    /** How many records the view created, for the counts of its filters. */
    @state accessor added = 0;
    /** What the groups' counts gained since read, by group: records created in them. */
    @state accessor countDeltas = new Map<string, number>();
    /** The groups those deltas apply to: read again, the counts are right of themselves. */
    private deltasOf: Group[] | null = null;

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

    /** The attributes of the `<list>` element, whose decorations colour the rows. */
    @computed get listAttrs(): Record<string, string> {
        return Object.fromEntries(Array.from(this.archRoot?.attributes ?? [], (attr) => [attr.name, attr.value]));
    }

    /** The fields read with the rows besides those shown: what their decorations read. */
    @computed get extraNames(): string[] {
        const fields = this.fields ?? {};
        const names = [
            ...decorationNames(this.listAttrs),
            ...this.columns.flatMap((column) => decorationNames(column.attrs)),
            ...companionFields(this.columns, fields),
        ];
        return [...new Set(names)].filter((name) => name in fields && !this.columns.some((column) => column.name === name));
    }

    /** The class of a record's row: selected, and coloured as the list's decorations say. */
    rowClass(record: Values): string {
        const decoration = decorationOf(this.listAttrs, record);
        return [
            "o_list_row",
            this.grouping !== null ? "o_list_grouped" : "",
            this.selected.has(this.idOf(record)) ? "selected" : "",
            decoration === null ? "" : `o_list_decoration_${decoration}`,
        ]
            .filter(Boolean)
            .join(" ");
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
        rememberList(listKey(this.router.route), {
            offset: this.grouping === null ? this.offset : 0,
            selected: [...this.selected],
            facets: this.currentFacets,
            sort: this.sort,
            ids: shown,
            total: this.grouping === null ? (this.total ?? 0) : shown.length,
            domain: this.domain,
            order: this.sort === null ? undefined : [`${this.sort.name} ${this.sort.descending ? "desc" : "asc"}`],
            groupBy: this.grouping?.groupBy ?? null,
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

    /** The searches the user saved on this list. */
    @resource accessor favorites: Favorite[] = load(
        () => this.router.route.action,
        (action) => favoritesOf(this.orm, action),
    );

    /**
     * The search as the user left it; until they touch it, the one they open the list with, or the
     * view's — none for records opened from another, such as an order's deliveries: all of them.
     */
    get currentFacets(): Facet[] {
        if (this.facets === null && this.router.route.ids) {
            return [];
        }
        return this.facets ?? this.favorites?.find((favorite) => favorite.is_default)?.facets ?? defaultFacets(this.searchView);
    }

    /** Save the search as it stands under a name, the list opening with it if asked. */
    readonly saveFavorite = async (name: string, isDefault: boolean): Promise<void> => {
        const action = this.router.route.action;
        if (await saveFavorite(this.orm, this.notifications, action, name, this.currentFacets, isDefault)) {
            refresh(() => this.favorites);
        }
    };

    readonly forgetFavorite = async (favorite: Favorite): Promise<void> => {
        await forgetFavorite(this.orm, favorite);
        refresh(() => this.favorites);
    };

    readonly applyFavorite = (favorite: Favorite): void => {
        this.setFacets([...favorite.facets]);
    };

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

    /**
     * The field the groups are of, when it is one the view's `group_create` lists: each group then
     * offers to create a record in it, from its title, which takes the group's value.
     */
    @computed get groupCreateField(): string | null {
        const groupBy = this.grouping?.groupBy ?? null;
        if (groupBy === null || groupBy.includes(":")) {
            return null;
        }
        return groupCreateFields(this.archRoot, this.fields).includes(groupBy) ? groupBy : null;
    }

    /** Open a group to create a record in it from its title, at its top. */
    async openAdd(group: Group): Promise<void> {
        if (!this.isOpen(group)) {
            await this.toggleGroup(group);
        }
        this.adding = this.groupKey(group);
        await nextTick();
        document.querySelector<HTMLInputElement>(".o_list_quick input")?.focus();
    }

    readonly closeAdd = (): void => {
        this.adding = null;
    };

    /**
     * Create a record in a group from the title typed: at once, its row then focused, when the
     * title is enough; else completed in a form first.
     */
    async add(group: Group, form: HTMLFormElement): Promise<void> {
        const field = this.groupCreateField;
        const title = titleField(this.archRoot, this.fields);
        const text = form.querySelector("input")?.value.trim() ?? "";
        if (field === null || title === null || text === "") {
            return;
        }
        const values: Values = { ...this.props.defaults, [title]: text, [field]: groupValue(group.value) };
        this.adding = null;
        if (await needsForm(this.orm, this.props.resModel, this.fields ?? {}, values)) {
            this.completing = { ...values, [field]: group.value };
            this.completingGroup = group;
            return;
        }
        try {
            const [id] = await this.orm.create(this.props.resModel, values);
            await this.createdIn(group);
            await this.focusRow(id);
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
        }
    }

    readonly completed = ([id]: [number, string | null]): void => {
        const group = this.completingGroup;
        if (group !== null) {
            void this.createdIn(group).then(() => this.focusRow(id));
        }
    };

    readonly closeForm = (): void => {
        this.completing = null;
    };

    /**
     * A record created in a group: counted there and among the view's without counting them
     * again, and the group's records read again to show it.
     */
    private async createdIn(group: Group): Promise<void> {
        const key = this.groupKey(group);
        const groups = this.groups ?? null;
        const deltas = this.deltasOf === groups ? new Map(this.countDeltas) : new Map<string, number>();
        deltas.set(key, (deltas.get(key) ?? 0) + 1);
        this.deltasOf = groups;
        this.countDeltas = deltas;
        this.added += 1;
        this.openGroups.delete(key);
        await this.toggleGroup(group);
    }

    /** How many records a group holds, with those created in it since read. */
    groupCount(group: Group): number {
        const delta = this.deltasOf === (this.groups ?? null) ? (this.countDeltas.get(this.groupKey(group)) ?? 0) : 0;
        return group.count + delta;
    }

    /** Bring a row just created into view and focus it, once shown. */
    private async focusRow(id: number): Promise<void> {
        for (let attempt = 0; attempt < 40; attempt++) {
            const row = document.querySelector<HTMLElement>(`.o_list_row[data-id="${id}"]`);
            if (row !== null) {
                row.scrollIntoView({ block: "nearest" });
                row.focus();
                return;
            }
            await new Promise((resolve) => setTimeout(resolve, 50));
        }
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
            const quick: Line[] = this.adding === key ? [{ kind: "quick", key: `${key}/quick`, group }] : [];
            return [{ kind: "group", key, group }, ...quick, ...records];
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
        const fields = [...this.columns.map((column) => column.name), ...this.extraNames];
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
            fields: [...this.columns.map((column) => column.name), ...this.extraNames],
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
                    await this.router.go(
                        opened.ids === null
                            ? { action: opened.action, view: "form", id: opened.id }
                            : { action: opened.action, view: "list", id: null, ids: opened.ids },
                    );
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

    /** Whether what the search and its groupings depend on is still on its way. */
    private get searchLoading(): boolean {
        return loading(() => this.favorites) || loading(() => this.searchArch) || loading(() => this.fields);
    }

    /**
     * What the pager says: the rows shown and how many match, or how many groups. Read without
     * waiting on anything, so the pager shows `…` meanwhile rather than holding the bar back.
     */
    get pagerText(): string {
        if (this.searchLoading) {
            return "…";
        }
        if (this.grouping !== null) {
            return loading(() => this.groups) ? "…" : `${this.groups?.length ?? 0} groups`;
        }
        return loading(() => this.total) ? "…" : `${this.range} / ${this.total}`;
    }

    /** Whether the rows come a page at a time: not while they are grouped. */
    get pagesTurn(): boolean {
        return !this.searchLoading && this.grouping === null;
    }

    get hasPrevious(): boolean {
        return this.offset > 0;
    }

    get hasNext(): boolean {
        return !loading(() => this.total) && this.offset + this.props.limit < (this.total ?? 0);
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
export function periodLabel(day: string, period: string): string {
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
