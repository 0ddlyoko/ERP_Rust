import { Component, type ComponentClass, computed, inject, load, loading, props, resource, state, t } from "trame";
import { and } from "@web/core/domain";
import { Notifications } from "@web/core/notifications";
import type { Domain, Group, Values } from "@web/core/orm";
import { FilterChips } from "@web/views/search/filter_chips";
import { SearchBar } from "@web/views/search/search_bar";
import { defaultFacets, type Facet, groupByOf, readSearchView, type SearchView, searchDomain } from "@web/views/search/search_model";
import { type Column, View, viewKinds, viewProps, widgetFor } from "@web/views/view";
import { companionFields } from "@web/views/widgets/decimal_widget";

/** A column of cards: the group it shows and its records. */
interface Lane {
    key: string;
    label: string;
    count: number;
    records: Values[];
}

/** A record as a card: its title, then the fields under it, each with its widget. */
export class KanbanCard extends Component {
    static template = "web.KanbanCard";

    props = props({
        record: t.object(),
        model: t.string(),
        title: t.any<Column>().optional(),
        details: t.array(t.any<Column>()),
        widgetFor: t.func<(column: Column) => ComponentClass>(),
        onOpen: t.func<(record: Values) => void>(),
    });

    get fieldClass(): (column: Column) => string {
        return (column) => `o_kanban_card_field o_field_${column.field.type}`;
    }
}

/**
 * Records as cards: the first field of the view's XML as their title, the others under it.
 *
 * Gathered in columns by the field `default_group_by` names, or the one the search groups by —
 * a value each, not a date's period — each column saying how many records it holds; otherwise
 * laid out side by side. Choosing a card opens its record.
 */
export class KanbanView extends View {
    static template = "web.KanbanView";
    static components = { FilterChips, KanbanCard, SearchBar };

    override props = props({
        ...viewProps,
        /** How many records are shown at most. */
        limit: t.number().default(200),
    });

    @inject(Notifications) notifications!: Notifications;

    override get kind(): string {
        return "kanban";
    }

    @state accessor facets: Facet[] | null = null;

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
        const view = readSearchView(arch, fields);
        return { ...view, groupable: view.groupable.filter((option) => !option.groupBy.includes(":")) };
    }

    /** The search the user made; until then the view's — none for records opened from another. */
    get currentFacets(): Facet[] {
        if (this.facets === null && this.router.route.ids) {
            return [];
        }
        return this.facets ?? defaultFacets(this.searchView);
    }

    readonly setFacets = (facets: Facet[]): void => {
        this.facets = facets;
    };

    @computed get domain(): Domain {
        return and([[...this.props.domain], searchDomain(this.searchView, this.currentFacets)]);
    }

    /** The field the cards are gathered by: the search's, else the view's `default_group_by`. */
    @computed get groupField(): string | null {
        const searched = groupByOf(this.currentFacets)?.groupBy ?? null;
        const field = searched ?? this.archRoot?.getAttribute("default_group_by") ?? null;
        return field === null || field.includes(":") ? null : field;
    }

    /** The card's title, then what it shows under it. */
    @computed get title(): Column | undefined {
        return this.columns[0];
    }

    @computed get details(): Column[] {
        return this.columns.slice(1);
    }

    @resource accessor groups: Group[] | null = load(
        () => ({ model: this.props.resModel, domain: this.domain, groupBy: this.groupField }),
        ({ model, domain, groupBy }) => (groupBy === null ? Promise.resolve(null) : this.orm.readGroup(model, domain, groupBy)),
    );

    @resource accessor records: Values[] = load(
        () => ({
            model: this.props.resModel,
            domain: this.domain,
            fields: [
                ...new Set([
                    ...this.columns.map((column) => column.name),
                    ...(this.groupField === null ? [] : [this.groupField]),
                    ...companionFields(this.columns, this.fields ?? {}),
                ]),
            ],
            limit: this.props.limit,
        }),
        ({ model, domain, fields, limit }) => this.orm.searchRead(model, domain, fields, { limit, names: true }),
    );

    /** The columns, in the order of the field's values — a selection's as it lists them. */
    @computed get lanes(): Lane[] {
        const name = this.groupField;
        const groups = this.groups;
        if (name === null || groups === null || groups === undefined) {
            return [];
        }
        const field = this.fields?.[name];
        const order = (field?.values ?? []).map(([key]) => key);
        const rank = (group: Group): number => {
            const at = order.indexOf(group.value as string);
            return at < 0 ? order.length : at;
        };
        const sorted = field?.type === "selection" ? [...groups].sort((left, right) => rank(left) - rank(right)) : groups;
        const keyOf = (value: unknown): string => JSON.stringify(Array.isArray(value) ? value[0] : (value ?? null));
        return sorted.map((group) => ({
            key: keyOf(group.value),
            label: this.labelOf(group.value),
            count: group.count,
            records: (this.records ?? []).filter((record) => keyOf(record[name]) === keyOf(group.value)),
        }));
    }

    /** A column's heading: its value as the field shows it, `None` for none. */
    labelOf(value: unknown): string {
        const field = this.fields?.[this.groupField ?? ""];
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
        return String(value);
    }

    /** Whether the cards are shown in columns; the bar shows how many records otherwise. */
    get grouped(): boolean {
        return !loading(() => this.searchArch) && !loading(() => this.fields) && this.groupField !== null;
    }

    readonly cardWidget = (column: Column): ComponentClass =>
        column.field.type === "refs" ? widgetFor(column, "tags") : widgetFor(column);

    readonly open = (record: Values): void => {
        this.router.go({ ...this.router.route, view: "form", id: record.id as number });
    };

    create(): void {
        this.router.go({ ...this.router.route, view: "form", id: null });
    }
}

viewKinds.add("kanban", KanbanView);
