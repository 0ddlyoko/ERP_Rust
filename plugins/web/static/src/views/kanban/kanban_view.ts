import {
    Component,
    type ComponentClass,
    computed,
    effect,
    inject,
    load,
    loading,
    nextTick,
    props,
    refresh,
    resource,
    state,
    t,
    untrack,
} from "trame";
import { and } from "@web/core/domain";
import { decorationNames, decorationOf, evaluate, namesRead } from "@web/core/expression";
import { Notifications } from "@web/core/notifications";
import type { Domain, Group, Values } from "@web/core/orm";
import { Session } from "@web/core/session";
import { FormDialog } from "@web/views/form/form_dialog";
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
import { type CompiledCard, compileCard } from "@web/views/strip/card_compiler";
import type { CardHost } from "@web/views/strip/card_body";
import { groupCreateFields, groupValue, isEmptyValue, needsForm, titleField } from "@web/views/group_create";
import { type Column, View, viewKinds, viewProps, widgetFor } from "@web/views/view";
import { companionFields } from "@web/views/widgets/decimal_widget";

/**
 * A card of a column: one of its records, the dragged card where it was — hidden, its place
 * given up — or, `ghost`, the dragged card greyed where it would land.
 */
interface Slot {
    key: string;
    record: Values;
    hidden: boolean;
    ghost: boolean;
}

/** A column a view shows: the value its cards share, its heading, and whether it starts folded. */
interface LaneHead {
    key: string;
    /** The value written to a card dropped here: a record's id, a selection's key, `null`. */
    value: unknown;
    label: string;
    folded: boolean;
    /** Where it stands among the columns, for those that keep an order. */
    sequence?: number;
    /** What finds its cards among the view's. */
    domain?: Domain;
}

/** A column of cards, as shown. */
interface Lane extends LaneHead {
    count: number;
    records: Values[];
    slots: Slot[];
}

/** Every column a record could go to, and how to change them: the records of a model. */
interface Board {
    heads: LaneHead[];
    /** The model of the columns, the field naming them, and whether they keep an order. */
    model: string;
    label: string;
    sequenced: boolean;
    /** What a new column starts with: what narrows them to this board — its project. */
    defaults: Values;
}

/** Records of a many2many as a widget changes them: their ids, for writing. */
function idsOfValue(value: unknown): number[] {
    return (Array.isArray(value) ? value : []).flatMap((item: unknown) => {
        if (Array.isArray(item)) {
            return [item[0] as number];
        }
        if (item !== null && typeof item === "object" && typeof (item as { id?: unknown }).id === "number") {
            return [(item as { id: number }).id];
        }
        return typeof item === "number" ? [item] : [];
    });
}

/** The same as `[id, name]`, for showing them. */
function namedOfValue(value: unknown): unknown[] {
    return (Array.isArray(value) ? value : []).flatMap((item: unknown) => {
        if (Array.isArray(item)) {
            return [item];
        }
        if (item !== null && typeof item === "object" && typeof (item as { id?: unknown }).id === "number") {
            const { id, name } = item as { id: number; name?: string | null };
            return [[id, name ?? null]];
        }
        return [];
    });
}

/** How many cards a column shows, then as many more each time more are asked for. */
const LANE_LIMIT = 100;
/** How many columns show open; those after them start folded. */
const OPEN_LANES = 15;

/** How long a column takes to fold away when deleted, in milliseconds. */
const LEAVE_MS = 200;

/** Whether the user asked for less motion. */
function stillness(): boolean {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * An order between two neighbours' — one of them missing at an end of the column — or `null`
 * when they leave no whole number between them.
 */
function sequenceBetween(before: number | null, after: number | null): number | null {
    if (before === null && after === null) {
        return 10;
    }
    if (before === null) {
        return (after as number) - 10;
    }
    if (after === null) {
        return before + 10;
    }
    return after - before > 1 ? Math.floor((before + after) / 2) : null;
}

/** Where each element matching a selector stands, by the value of one of its data attributes. */
function places(selector: string, key: string): Map<string, DOMRect> {
    return new Map(
        Array.from(document.querySelectorAll<HTMLElement>(selector)).map((element) => [
            element.dataset[key] ?? "",
            element.getBoundingClientRect(),
        ]),
    );
}

/** Slide each element that moved from where it stood to where it now is. */
function slideFrom(before: Map<string, DOMRect>, selector: string, key: string): void {
    if (stillness()) {
        return;
    }
    for (const element of document.querySelectorAll<HTMLElement>(selector)) {
        const from = before.get(element.dataset[key] ?? "");
        if (from === undefined) {
            continue;
        }
        const to = element.getBoundingClientRect();
        const dx = from.left - to.left;
        const dy = from.top - to.top;
        if (Math.abs(dx) > 1 || Math.abs(dy) > 1) {
            element.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }], {
                duration: 240,
                easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
            });
        }
    }
}

/** What follows the pointer while an element is dragged: a copy of it, tilted by some degrees. */
function tiltedImage(event: DragEvent, element: HTMLElement, degrees: number): void {
    if (event.dataTransfer === null) {
        return;
    }
    const box = element.getBoundingClientRect();
    const holder = document.createElement("div");
    holder.className = "o_kanban_drag_image";
    holder.style.width = `${box.width + 40}px`;
    const copy = element.cloneNode(true) as HTMLElement;
    copy.style.width = `${box.width}px`;
    copy.style.transform = `rotate(${stillness() ? 0 : degrees}deg)`;
    holder.append(copy);
    document.body.append(holder);
    event.dataTransfer.setDragImage(holder, event.clientX - box.left + 20, event.clientY - box.top + 20);
    setTimeout(() => holder.remove(), 0);
}

/** The key of a value, the same for a record as `[id, name]` and as its id. */
function keyOf(value: unknown): string {
    return JSON.stringify(Array.isArray(value) ? value[0] : (value ?? null));
}

/**
 * A record as a card: as the view's `<card>` lays it out, or else its title — the first field of
 * the view's XML — and the fields under it, each with its widget.
 */
export class KanbanCard extends Component {
    static template = "web.KanbanCard";

    props = props({
        record: t.object(),
        model: t.string(),
        card: t.any<CompiledCard>().optional(),
        host: t.any<CardHost>().optional(),
        title: t.any<Column>().optional(),
        details: t.array(t.any<Column>()),
        widgetFor: t.func<(column: Column) => ComponentClass>(),
        onOpen: t.func<(record: Values) => void>(),
        draggable: t.boolean().default(false),
        /** The colour the card's `decoration-*` attributes give it, for its record. */
        decoration: t.string().orNull().default(null),
        dragging: t.boolean().default(false),
        /** Shown greyed where the card being dragged would land. */
        ghost: t.boolean().default(false),
        onDragStart: t.func<(record: Values, event: DragEvent) => void>().optional(),
        onDragEnd: t.func<() => void>().optional(),
    });

    get fieldClass(): (column: Column) => string {
        return (column) => `o_kanban_card_field o_field_${column.field.type}`;
    }
}

/**
 * Records as cards, laid out as the view's `<card>` says — or the first field of its XML as their
 * title, the others under it. The `<card>`'s `decoration-*` attributes colour a card's edge, as
 * they colour a list's rows.
 *
 * Gathered in columns by the field `default_group_by` names, or the one the search groups by —
 * a value each, not a date's period — each column saying how many records it holds; otherwise
 * laid out side by side. Choosing a card opens its record; with `open_action` and `open_by`, it
 * opens instead that action's records belonging to it — a project's board of tasks — unless
 * `open_form`, an expression of the record, holds: a project template opens in its form.
 *
 * Gathered by a field `group_create` lists, a card is dragged to another column to change it, and
 * within a column to reorder the cards, when the records have a `sequence`. With `expand="1"`,
 * every column a record could go to shows, empty or not: the records of the field's model sharing
 * what the view's records start with — the columns of the project of a board. `group_fold` names
 * the field of those records saying a column starts folded; any column folds and unfolds by hand.
 * A card is created in such a column from its title — the field `quick_create` names, else the
 * model's name — at the top or the bottom of it; `quick_create_string` says what the buttons
 * doing it offer to add. A record the
 * title is enough for is created at once, and its card focused; one asking for more opens in a
 * form, the title filled in.
 *
 * With `group_edit` naming a group the user is in, the columns themselves change from the board:
 * one is added at its end, renamed by a click on its title, dragged by its heading among the
 * others, which numbers them anew, and deleted — its cards then in a column of no name, which
 * becomes a column of the name given to it.
 *
 * What moves is seen moving: a card or a column dragged follows the pointer, tilted; once dropped,
 * the cards and columns slide from where they were to where they now are.
 */
export class KanbanView extends View implements CardHost {
    static template = "web.KanbanView";
    static components = { FilterChips, FormDialog, KanbanCard, SearchBar };

    override props = props({
        ...viewProps,
        /** How many records are shown at most. */
        limit: t.number().default(200),
    });

    @inject(Notifications) notifications!: Notifications;
    @inject(Session) session!: Session;

    override get kind(): string {
        return "kanban";
    }

    @state accessor facets: Facet[] | null = null;
    /** The columns folded or unfolded by hand, against how they start. */
    @state accessor toggled = new Set<string>();
    /** The record of the card being dragged. */
    @state accessor dragging: number | null = null;
    /** Where it would land: a column, and a position among its other cards. */
    @state accessor dropAt: { lane: string; index: number } | null = null;
    /** How many records the view created, for the counts of its filters. */
    @state accessor added = 0;
    /** What the columns' counts gained or lost since read: cards created and moved. */
    @state accessor countDeltas = new Map<string, number>();
    /** The groups those deltas apply to: read again, the counts are right of themselves. */
    private deltasOf: Group[] | null = null;
    /** Where a card was dropped, kept there while its column and place are saved. */
    @state accessor placed: { id: number; lane: string; index: number } | null = null;
    /** How many cards each column shows, when more were asked for than it starts with. */
    @state accessor laneLimits = new Map<string, number>();
    /** A field of a card being changed from it, in an editor beside it. */
    @state accessor quick: { record: Values; column: Column; place: string } | null = null;
    /** The column a card is being created in, from its title, and whether at its top or bottom. */
    @state accessor adding: { lane: string; at: "top" | "bottom" } | null = null;
    /** What a card whose title is not enough starts with, while a form completes it. */
    @state accessor completing: Values | null = null;
    /** The column that card is created in. */
    private completingLane: string | null = null;
    /** The column whose title is being changed. */
    @state accessor renaming: string | null = null;
    /** Whether a column is being added at the end of the board. */
    @state accessor addingColumn = false;
    /** The column being dragged by its heading, and the one it would land before — `null` for the end. */
    @state accessor draggingLane: string | null = null;
    @state accessor laneDropBefore: string | null | undefined = undefined;
    /** The columns being deleted, folding away before they go. */
    @state accessor leaving = new Set<string>();

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

    /** The searches the user saved on these records. */
    @resource accessor favorites: Favorite[] = load(
        () => this.router.route.action,
        (action) => favoritesOf(this.orm, action),
    );

    /**
     * The search the user made; until then the one they open the view with, or the view's — none
     * for records opened from another.
     */
    get currentFacets(): Facet[] {
        if (this.facets === null && this.router.route.ids) {
            return [];
        }
        const opening = loading(() => this.favorites) ? undefined : this.favorites?.find((favorite) => favorite.is_default);
        return this.facets ?? opening?.facets ?? defaultFacets(this.searchView);
    }

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

    /** The saved searches, none while they are read. */
    get loadedFavorites(): Favorite[] {
        return loading(() => this.favorites) ? [] : (this.favorites ?? []);
    }

    readonly applyFavorite = (favorite: Favorite): void => {
        this.setFacets([...favorite.facets]);
    };

    readonly setFacets = (facets: Facet[]): void => {
        this.facets = facets;
    };

    @computed get domain(): Domain {
        return and([[...this.props.domain], searchDomain(this.searchView, this.currentFacets)]);
    }

    private attribute(name: string): string | null {
        return this.archRoot?.getAttribute(name) ?? null;
    }

    /** The field the cards are gathered by: the search's, else the view's `default_group_by`. */
    @computed get groupField(): string | null {
        const searched = groupByOf(this.currentFacets)?.groupBy ?? null;
        const field = searched ?? this.attribute("default_group_by");
        return field === null || field.includes(":") ? null : field;
    }

    /** The view's `<card>`, as a template. */
    @computed get card(): CompiledCard | null {
        const root = this.archRoot;
        const fields = this.fields;
        const element = root === undefined ? undefined : Array.from(root.children).find((child) => child.tagName === "card");
        if (element === undefined || fields === undefined) {
            return null;
        }
        return compileCard(element, (child) => this.columnOf(child, fields), true);
    }

    /** The card's title, then what it shows under it, without a `<card>`. */
    @computed get title(): Column | undefined {
        return this.columns[0];
    }

    @computed get details(): Column[] {
        return this.columns.slice(1);
    }

    /** Whether the records are kept in an order of their own, which dragging a card changes. */
    get hasSequence(): boolean {
        return this.fields?.sequence !== undefined;
    }

    /** Whether a condition of a card holds for a record. */
    holds(record: Values, expression: string): boolean {
        return !!evaluate(expression, record);
    }

    @resource accessor groups: Group[] | null = load(
        () => ({ model: this.props.resModel, domain: this.domain, groupBy: this.groupField }),
        ({ model, domain, groupBy }) => (groupBy === null ? Promise.resolve(null) : this.orm.readGroup(model, domain, groupBy)),
    );

    /** The attributes of the view's `<card>`, whose decorations colour the cards. */
    @computed get cardAttrs(): Record<string, string> {
        const element = this.archRoot === undefined ? undefined : Array.from(this.archRoot.children).find((child) => child.tagName === "card");
        return Object.fromEntries(Array.from(element?.attributes ?? [], (attr) => [attr.name, attr.value]));
    }

    /** The colour a card takes, as its `decoration-*` attributes say: a blocked task's, a late one's. */
    cardDecoration(record: Values): string | null {
        return decorationOf(this.cardAttrs, record);
    }

    /** What a card is read with: the fields its card shows and colours it, its column's, its order. */
    @computed get cardFields(): string[] {
        const shown = this.card?.columns ?? this.columns;
        const fields = this.fields ?? {};
        return [
            ...new Set([
                ...shown.map((column) => column.name),
                ...decorationNames(this.cardAttrs).filter((name) => name in fields),
                ...namesRead(this.attribute("open_form") ?? "").filter((name) => name in fields),
                ...(this.groupField === null ? [] : [this.groupField]),
                ...(this.hasSequence ? ["sequence"] : []),
                ...companionFields(shown, this.fields ?? {}),
            ]),
        ];
    }

    /** The cards read, by column — `""` for those of a view not gathered in columns. */
    @state accessor laneCards = new Map<string, Values[]>();
    /** How many columns are being read. */
    @state accessor readingLanes = 0;
    /** Raised, by column, to read its cards again. */
    @state accessor laneVersions = new Map<string, number>();
    /** What each column was last read with, so that only a column whose read changed is read. */
    private laneReads = new Map<string, string>();
    /** The latest read asked of each column: an answer overtaken by a later one is dropped. */
    private laneAsked = new Map<string, number>();

    /**
     * What to read of the cards: column by column when gathered — each its first hundred, or as
     * many as were asked for, a folded one none until unfolded, an empty one none until a card is
     * dropped or added there — else the view's first ones.
     * `null` while the columns are still being found.
     */
    @computed get cardReads(): { lane: string; domain: Domain; limit: number }[] | null {
        if (this.archRoot === undefined || loading(() => this.searchArch) || loading(() => this.fields)) {
            return null;
        }
        if (this.groupField === null) {
            return [{ lane: "", domain: this.domain, limit: this.props.limit }];
        }
        const groups = loading(() => this.groups) ? null : this.groups;
        if (groups === null || groups === undefined || loading(() => this.board)) {
            return null;
        }
        const counts = new Map(groups.map((group) => [keyOf(group.value), group.count]));
        const counted = this.countDeltas;
        const deltas = this.deltasOf === groups ? counted : new Map<string, number>();
        const holds = (lane: string): boolean => (counts.get(lane) ?? 0) + (deltas.get(lane) ?? 0) > 0;
        return this.heads
            .filter((head) => !head.folded && holds(head.key) && head.domain !== undefined)
            .map((head) => ({
                lane: head.key,
                domain: and([[...this.domain], [...(head.domain as Domain)]]),
                limit: this.laneLimits.get(head.key) ?? LANE_LIMIT,
            }));
    }

    /**
     * Read the cards of each column whose read changed — its search, how many it shows — or that
     * was asked to be read again; the others keep theirs, and one no longer read — folded, gone —
     * forgets its own. A column is always read from its first card, so that cards added or
     * removed meanwhile neither repeat nor go missing.
     */
    @effect readCards(): void {
        const reads = this.cardReads;
        const versions = this.laneVersions;
        const fields = this.cardFields;
        const order = this.hasSequence ? ["sequence", "id"] : undefined;
        if (reads === null || this.fields === undefined) {
            return;
        }
        const model = this.props.resModel;
        untrack(() => {
            const kept = new Set(reads.map(({ lane }) => lane));
            const forgotten = [...this.laneCards.keys()].filter((lane) => !kept.has(lane));
            if (forgotten.length > 0) {
                const cards = new Map(this.laneCards);
                for (const lane of forgotten) {
                    cards.delete(lane);
                    this.laneReads.delete(lane);
                    this.laneAsked.set(lane, (this.laneAsked.get(lane) ?? 0) + 1);
                }
                this.laneCards = cards;
            }
            for (const { lane, domain, limit } of reads) {
                const signature = JSON.stringify([model, domain, limit, fields, order, versions.get(lane) ?? 0]);
                if (this.laneReads.get(lane) === signature) {
                    continue;
                }
                this.laneReads.set(lane, signature);
                const asked = (this.laneAsked.get(lane) ?? 0) + 1;
                this.laneAsked.set(lane, asked);
                this.readingLanes += 1;
                void this.orm
                    .searchRead(model, domain, fields, { limit, order, names: true })
                    .then((found) => {
                        if (this.laneAsked.get(lane) === asked) {
                            this.laneCards = new Map(this.laneCards).set(lane, found);
                        }
                    })
                    .catch((error) => this.notifications.add("danger", error instanceof Error ? error.message : String(error)))
                    .finally(() => {
                        this.readingLanes -= 1;
                    });
            }
        });
    }

    /** The cards read so far, column after column. */
    get records(): Values[] {
        return [...this.laneCards.values()].flat();
    }

    /** Read some columns' cards again — every one without saying which. */
    private rereadLanes(lanes?: string[]): void {
        const versions = new Map(this.laneVersions);
        for (const lane of lanes ?? [...this.laneCards.keys()]) {
            versions.set(lane, (versions.get(lane) ?? 0) + 1);
        }
        this.laneVersions = versions;
    }

    /** Show a card in another column at once, at a place, while it is saved there. */
    private moveCard(id: number, from: string, to: string, index: number, values: Values): void {
        const cards = new Map(this.laneCards);
        const card = (cards.get(from) ?? []).find((one) => one.id === id);
        if (card === undefined) {
            return;
        }
        cards.set(from, (cards.get(from) ?? []).filter((one) => one.id !== id));
        const target = (cards.get(to) ?? []).filter((one) => one.id !== id);
        target.splice(Math.min(index, target.length), 0, { ...card, ...values });
        cards.set(to, target);
        this.laneCards = cards;
    }

    /** How many cards each column holds, as read. */
    get baseCounts(): Map<string, number> | null {
        const groups = loading(() => this.groups) ? null : this.groups;
        return groups === null || groups === undefined ? null : new Map(groups.map((group) => [keyOf(group.value), group.count]));
    }

    /** Show more of a column's cards. */
    showMore(lane: Lane): void {
        const limits = new Map(this.laneLimits);
        limits.set(lane.key, (limits.get(lane.key) ?? LANE_LIMIT) + LANE_LIMIT);
        this.laneLimits = limits;
    }

    /**
     * Every column a record could go to, with `expand="1"`: the records of the model the cards are
     * gathered by that share what a new card starts with — its project's columns. None when
     * nothing narrows them, rather than every column of every project.
     */
    @resource accessor board: Board | null = load(
        () => {
            const name = this.groupField;
            const relation = name === null ? undefined : this.fields?.[name]?.relation;
            if (this.attribute("expand") !== "1" || relation === undefined) {
                return null;
            }
            return { relation, defaults: this.props.defaults, fold: this.attribute("group_fold") };
        },
        async (asked) => {
            if (asked === null) {
                return null;
            }
            const fields = await this.models.fields(asked.relation);
            const label = Object.keys(fields).find((field) => fields[field].name_field) ?? "name";
            const domain = Object.entries(asked.defaults)
                .filter(([field]) => fields[field]?.relation !== undefined)
                .map(([field, value]) => [field, "=", groupValue(value)]);
            if (domain.length === 0) {
                return null;
            }
            const fold = asked.fold !== null && fields[asked.fold] !== undefined ? asked.fold : null;
            const sequenced = "sequence" in fields;
            const rows = await this.orm.searchRead(
                asked.relation,
                domain,
                [label, ...(fold === null ? [] : [fold]), ...(sequenced ? ["sequence"] : [])],
                { order: sequenced ? ["sequence", "id"] : ["id"] },
            );
            const heads = rows.map((row) => ({
                key: keyOf(row.id),
                value: row.id,
                label: String(row[label] ?? `#${row.id}`),
                folded: fold !== null && !!row[fold],
                sequence: Number(row.sequence) || 0,
            }));
            const defaults = Object.fromEntries(domain.map(([field, , value]) => [field as string, value]));
            return { heads, model: asked.relation, label, sequenced, defaults };
        },
    );

    /**
     * The columns, without their cards: every one there may be, else those holding records, in
     * their order; those after the first fifteen start folded.
     */
    get heads(): LaneHead[] {
        const name = this.groupField;
        const groups = loading(() => this.groups) ? null : this.groups;
        if (name === null || groups === null || groups === undefined) {
            return [];
        }
        const board = loading(() => this.board) ? null : (this.board ?? null);
        const expanded = board === null ? null : board.heads.map((head) => ({ ...head, domain: [[name, "=", head.value]] }));
        const heads: LaneHead[] = expanded === null ? this.headsOfGroups(groups) : expanded;
        if (expanded !== null) {
            for (const group of groups) {
                const key = keyOf(group.value);
                if (!heads.some((head) => head.key === key)) {
                    const label = isEmptyValue(group.value) ? "" : this.labelOf(group.value);
                    heads.unshift({ key, value: group.value, label, folded: false, domain: group.domain });
                }
            }
        }
        return heads.map((head, index) => ({
            ...head,
            folded: (head.folded || index >= OPEN_LANES) !== this.toggled.has(head.key),
        }));
    }

    /** The columns with their cards. */
    @computed get lanes(): Lane[] {
        const name = this.groupField;
        const groups = loading(() => this.groups) ? null : this.groups;
        const counts = this.baseCounts;
        if (name === null || groups === null || groups === undefined || counts === null) {
            return [];
        }
        const records = this.records;
        const heads = this.heads;
        const placed = this.placed;
        const laneOf = (record: Values): string =>
            placed !== null && placed.id === record.id ? placed.lane : keyOf(record[name]);
        const dragged = records.find((record) => record.id === this.dragging);
        return heads.map((head) => {
            const own = records.filter((record) => laneOf(record) === head.key);
            if (placed !== null && placed.lane === head.key) {
                const card = own.find((record) => record.id === placed.id);
                if (card !== undefined) {
                    own.splice(own.indexOf(card), 1);
                    own.splice(placed.index, 0, card);
                }
            }
            const at = this.dropAt?.lane === head.key ? this.dropAt.index : null;
            const ghost = (): Slot => ({ key: "ghost", record: dragged as Values, hidden: false, ghost: true });
            const slots: Slot[] = [];
            let index = 0;
            for (const record of own) {
                if (record.id === this.dragging) {
                    slots.push({ key: String(record.id), record, hidden: true, ghost: false });
                    continue;
                }
                if (at === index && dragged !== undefined) {
                    slots.push(ghost());
                }
                slots.push({ key: String(record.id), record, hidden: false, ghost: false });
                index++;
            }
            if (at !== null && at >= index && dragged !== undefined) {
                slots.push(ghost());
            }
            return {
                ...head,
                count: (counts.get(head.key) ?? 0) + (this.deltasOf === groups ? (this.countDeltas.get(head.key) ?? 0) : 0),
                records: own,
                slots,
            };
        });
    }

    /** The columns of the groups found, in the order of the field's values — a selection's as it lists them. */
    private headsOfGroups(groups: Group[]): LaneHead[] {
        const field = this.fields?.[this.groupField ?? ""];
        const order = (field?.values ?? []).map(([key]) => key);
        const rank = (group: Group): number => {
            const at = order.indexOf(group.value as string);
            return at < 0 ? order.length : at;
        };
        const sorted = field?.type === "selection" ? [...groups].sort((left, right) => rank(left) - rank(right)) : groups;
        return sorted.map((group) => ({
            key: keyOf(group.value),
            value: group.value,
            label: this.labelOf(group.value),
            folded: false,
            domain: group.domain,
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

    /**
     * Whether the cards' columns take cards — the field they are gathered by is one the view's
     * `group_create` lists: a card is dragged from one to another, and created in one from its
     * title.
     */
    get canDrag(): boolean {
        return this.grouped && this.groupField !== null && groupCreateFields(this.archRoot, this.fields).includes(this.groupField);
    }

    /** The field a card is created with from its title, in a column. */
    get quickCreate(): string | null {
        return this.canDrag ? titleField(this.archRoot, this.fields) : null;
    }

    /** What the button creating a card from its title offers to add. */
    get quickCreateString(): string {
        return this.attribute("quick_create_string") ?? "Add a card";
    }

    /** Whether the user adds, renames and moves the columns: they are in the view's `group_edit`. */
    get canEditColumns(): boolean {
        const group = this.attribute("group_edit");
        const board = loading(() => this.board) ? null : this.board;
        return group !== null && board !== null && board !== undefined && this.session.hasGroup(group);
    }

    async startRename(lane: Lane): Promise<void> {
        if (!this.canEditColumns) {
            return;
        }
        this.renaming = lane.key;
        await nextTick();
        document.querySelector<HTMLInputElement>(".o_kanban_lane_rename input")?.select();
    }

    readonly stopRename = (): void => {
        this.renaming = null;
    };

    /**
     * Give a column the title typed, once Enter is pressed or the field left; Escape keeps it.
     * Naming the cards of no column makes them a column of that name.
     */
    async rename(lane: Lane, form: HTMLFormElement | null): Promise<void> {
        if (this.renaming !== lane.key) {
            return;
        }
        const board = this.board;
        const text = form?.querySelector("input")?.value.trim() ?? "";
        this.renaming = null;
        if (!board || text === "" || text === lane.label) {
            return;
        }
        if (!isEmptyValue(lane.value)) {
            await this.changeColumns(() => this.orm.write(board.model, [groupValue(lane.value) as number], { [board.label]: text }));
            return;
        }
        const values: Values = { ...board.defaults, [board.label]: text };
        if (board.sequenced) {
            values.sequence = Math.min(10, ...board.heads.map((head) => head.sequence ?? 10)) - 10;
        }
        const name = this.groupField;
        const cards = lane.records.map((record) => record.id as number);
        await this.changeColumns(async () => {
            const [column] = await this.orm.create(board.model, values);
            if (name !== null && cards.length > 0) {
                await this.orm.write(this.props.resModel, cards, { [name]: column });
            }
        });
    }

    async openAddColumn(): Promise<void> {
        this.addingColumn = true;
        await nextTick();
        document.querySelector<HTMLInputElement>(".o_kanban_add_column input")?.focus();
    }

    readonly closeAddColumn = (): void => {
        this.addingColumn = false;
    };

    /** Add a column at the end of the board, of what narrows it — its project. */
    async addColumn(form: HTMLFormElement): Promise<void> {
        const board = this.board;
        const input = form.querySelector("input");
        const text = input?.value.trim() ?? "";
        if (!board || text === "") {
            return;
        }
        const values: Values = { ...board.defaults, [board.label]: text };
        if (board.sequenced) {
            values.sequence = Math.max(0, ...board.heads.map((head) => head.sequence ?? 0)) + 10;
        }
        await this.changeColumns(() => this.orm.create(board.model, values));
        if (input) {
            input.value = "";
        }
    }

    laneDragStart(lane: Lane, event: DragEvent): void {
        if (!this.canEditColumns || this.dragging !== null) {
            return;
        }
        event.stopPropagation();
        event.dataTransfer?.setData("text/plain", lane.key);
        if (event.dataTransfer) {
            event.dataTransfer.effectAllowed = "move";
        }
        const column = (event.currentTarget as HTMLElement).closest<HTMLElement>(".o_kanban_lane");
        if (column !== null) {
            tiltedImage(event, column, 2);
        }
        this.draggingLane = lane.key;
    }

    readonly laneDragEnd = (): void => {
        this.draggingLane = null;
        this.laneDropBefore = undefined;
    };

    /** Over the board, a dragged column lands among the others where the pointer is. */
    lanesDragOver(event: DragEvent): void {
        if (this.draggingLane === null) {
            return;
        }
        event.preventDefault();
        const lanes = Array.from((event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(".o_kanban_lane[data-key]"));
        const before = lanes.find((lane) => {
            const box = lane.getBoundingClientRect();
            return lane.dataset.key !== this.draggingLane && event.clientX < box.left + box.width / 2;
        });
        const key = before?.dataset.key ?? null;
        if (this.laneDropBefore !== key) {
            this.laneDropBefore = key;
        }
    }

    /** Put the dragged column where it landed, the columns numbered anew in their order. */
    async dropLane(event: DragEvent): Promise<void> {
        const board = this.board;
        const moved = this.draggingLane;
        const before = this.laneDropBefore;
        if (moved === null) {
            return;
        }
        event.preventDefault();
        this.laneDragEnd();
        if (!board || !board.sequenced || before === undefined) {
            return;
        }
        const heads = board.heads.filter((head) => head.key !== moved);
        const dragged = board.heads.find((head) => head.key === moved);
        if (dragged === undefined) {
            return;
        }
        const at = before === null ? heads.length : heads.findIndex((head) => head.key === before);
        heads.splice(at < 0 ? heads.length : at, 0, dragged);
        await this.changeColumns(() =>
            Promise.all(
                heads.flatMap((head, index) =>
                    head.sequence === (index + 1) * 10
                        ? []
                        : [this.orm.write(board.model, [groupValue(head.value) as number], { sequence: (index + 1) * 10 })],
                ),
            ),
        );
    }

    /** Change the columns, then show them as they now are, sliding into place; what fails is said. */
    private async changeColumns(change: () => Promise<unknown>): Promise<void> {
        const before = places(".o_kanban_lane[data-key]", "key");
        try {
            await change();
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
        }
        refresh(() => this.board);
        this.reload();
        await this.settled();
        slideFrom(before, ".o_kanban_lane[data-key]", "key");
    }

    /** Delete a column: it folds away, then goes; its cards stay, in a column of no name. */
    async deleteColumn(lane: Lane): Promise<void> {
        const board = this.board;
        if (!board || isEmptyValue(lane.value)) {
            return;
        }
        this.leaving = new Set([...this.leaving, lane.key]);
        await new Promise((resolve) => setTimeout(resolve, LEAVE_MS));
        await this.changeColumns(() => this.orm.delete(board.model, [groupValue(lane.value) as number]));
        const leaving = new Set(this.leaving);
        leaving.delete(lane.key);
        this.leaving = leaving;
    }

    /** Wait for what was asked again to be shown. */
    private async settled(): Promise<void> {
        await nextTick();
        for (let attempt = 0; attempt < 60; attempt++) {
            if (this.readingLanes === 0 && !loading(() => this.groups) && !loading(() => this.board)) {
                break;
            }
            await new Promise((resolve) => setTimeout(resolve, 25));
        }
        await nextTick();
    }

    readonly cardWidget = (column: Column): ComponentClass =>
        column.field.type === "refs" ? widgetFor(column, "tags") : widgetFor(column);

    readonly open = (record: Values): void => {
        const action = this.attribute("open_action");
        const by = this.attribute("open_by");
        const formFirst = this.attribute("open_form");
        if (formFirst !== null && evaluate(formFirst, record)) {
            this.openForm(record);
            return;
        }
        if (action !== null && by !== null) {
            void this.breadcrumb.openBy(action, by, record.id as number);
            return;
        }
        this.router.go({ ...this.router.route, view: "form", id: record.id as number });
    };

    /** Open a card's record in its form, whatever the card opens otherwise. */
    readonly openForm = (record: Values): void => {
        this.router.go({ ...this.router.route, view: "form", id: record.id as number });
    };

    create(): void {
        this.router.go({ ...this.router.route, view: "form", id: null });
    }

    toggleFold(lane: Lane): void {
        const toggled = new Set(this.toggled);
        if (!toggled.delete(lane.key)) {
            toggled.add(lane.key);
        }
        this.toggled = toggled;
    }

    readonly dragStart = (record: Values, event: DragEvent): void => {
        event.dataTransfer?.setData("text/plain", String(record.id));
        if (event.dataTransfer) {
            event.dataTransfer.effectAllowed = "move";
        }
        tiltedImage(event, event.currentTarget as HTMLElement, 3);
        setTimeout(() => {
            this.dragging = record.id as number;
        }, 0);
    };

    readonly dragEnd = (): void => {
        this.dragging = null;
        this.dropAt = null;
    };

    /** Over a column, a dragged card lands among the others where the pointer is. */
    dragOver(lane: Lane, event: DragEvent): void {
        if (this.dragging === null) {
            return;
        }
        event.preventDefault();
        const cards = Array.from(
            (event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(
                ".o_kanban_card:not(.o_kanban_dragging):not(.o_kanban_ghost)",
            ),
        );
        const index = cards.filter((card) => {
            const box = card.getBoundingClientRect();
            return box.top + box.height / 2 < event.clientY;
        }).length;
        if (this.dropAt?.lane !== lane.key || this.dropAt.index !== index) {
            this.dropAt = { lane: lane.key, index };
        }
    }

    dragLeave(lane: Lane, event: DragEvent): void {
        const into = event.relatedTarget as Node | null;
        if (this.dropAt?.lane === lane.key && !(event.currentTarget as HTMLElement).contains(into)) {
            this.dropAt = null;
        }
    }

    async drop(lane: Lane, event: DragEvent): Promise<void> {
        event.preventDefault();
        const id = this.dragging;
        const others = lane.records.filter((record) => record.id !== id).length;
        const index = this.dropAt?.lane === lane.key ? this.dropAt.index : others;
        if (id !== null) {
            this.placed = { id, lane: lane.key, index };
        }
        this.dragEnd();
        if (id !== null) {
            await this.move(id, lane, index);
        }
        this.placed = null;
    }

    /**
     * Put a card in a column, at a position among its other cards: its column written if it
     * changed, and, when the records keep an order, a number between its new neighbours' — the
     * column's cards numbered anew only when there is none left between them. The columns count
     * it once written, so that one empty until then is read with the card in it.
     */
    async move(id: number, lane: Lane, index: number): Promise<void> {
        const name = this.groupField;
        const record = (this.records ?? []).find((one) => one.id === id);
        if (name === null || record === undefined) {
            return;
        }
        const order = lane.records.filter((one) => one.id !== id);
        order.splice(Math.min(index, order.length), 0, record);
        const writes: Promise<boolean>[] = [];
        const moved: Values = {};
        const from = keyOf(record[name]);
        if (from !== lane.key) {
            moved[name] = groupValue(lane.value);
        }
        if (this.hasSequence) {
            const at = order.indexOf(record);
            const before = at > 0 ? Number(order[at - 1].sequence) : null;
            const after = at < order.length - 1 ? Number(order[at + 1].sequence) : null;
            const between = sequenceBetween(before, after);
            if (between !== null) {
                if (between !== record.sequence) {
                    moved.sequence = between;
                }
            } else {
                order.forEach((one, place) => {
                    const sequence = (place + 1) * 10;
                    if (one.id === id) {
                        moved.sequence = sequence;
                    } else if (one.sequence !== sequence) {
                        writes.push(this.orm.write(this.props.resModel, [one.id as number], { sequence }));
                    }
                });
            }
        }
        if (Object.keys(moved).length > 0) {
            writes.push(this.orm.write(this.props.resModel, [id], moved));
        }
        const before = places(".o_kanban_card[data-id]", "id");
        const sequence = moved.sequence === undefined ? {} : { sequence: moved.sequence };
        this.moveCard(id, from, lane.key, index, { [name]: lane.value, ...sequence });
        try {
            await Promise.all(writes);
            if (from !== lane.key) {
                this.bump(from, -1);
                this.bump(lane.key, 1);
            }
            this.rereadLanes(from === lane.key ? [from] : [from, lane.key]);
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
            this.reload();
        }
        await this.settled();
        slideFrom(before, ".o_kanban_card[data-id]", "id");
    }

    async openAdd(lane: Lane, at: "top" | "bottom"): Promise<void> {
        this.adding = { lane: lane.key, at };
        await nextTick();
        document.querySelector<HTMLInputElement>(".o_kanban_quick input")?.focus();
    }

    readonly closeAdd = (): void => {
        this.adding = null;
    };

    /**
     * Create a card from the title typed, where the column was asked to take it: created at once
     * and focused when the title is enough, else completed in a form first.
     */
    async add(form: HTMLFormElement): Promise<void> {
        const field = this.quickCreate;
        const text = form.querySelector("input")?.value.trim() ?? "";
        const lane = this.lanes.find((one) => one.key === this.adding?.lane);
        if (field === null || this.groupField === null || text === "" || lane === undefined) {
            return;
        }
        const values: Values = { ...this.props.defaults, [field]: text, [this.groupField]: groupValue(lane.value) };
        if (this.hasSequence) {
            const sequences = lane.records.map((one) => Number(one.sequence) || 0);
            values.sequence = this.adding?.at === "bottom" ? Math.max(0, ...sequences) + 10 : Math.min(10, ...sequences) - 1;
        }
        this.adding = null;
        if (await needsForm(this.orm, this.props.resModel, this.fields ?? {}, values)) {
            const named = Array.isArray(lane.value) ? lane.value : [lane.value, lane.label];
            const isRecord = this.fields?.[this.groupField]?.type === "ref";
            this.completing = { ...values, [this.groupField]: isRecord ? named : lane.value };
            this.completingLane = lane.key;
            return;
        }
        try {
            const [id] = await this.orm.create(this.props.resModel, values);
            this.counted(lane.key);
            this.rereadLanes([lane.key]);
            await this.focusCard(id);
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
        }
    }

    readonly completed = ([id]: [number, string | null]): void => {
        if (this.completingLane !== null) {
            this.counted(this.completingLane);
            this.rereadLanes([this.completingLane]);
        }
        void this.focusCard(id);
    };

    /** A record created in a column: one more there, and among the view's. */
    private counted(lane: string): void {
        this.bump(lane, 1);
        this.added += 1;
    }

    readonly closeForm = (): void => {
        this.completing = null;
    };

    /** Bring a card just created into view and focus it, once its column shows it. */
    private async focusCard(id: number): Promise<void> {
        for (let attempt = 0; attempt < 40; attempt++) {
            const card = document.querySelector<HTMLElement>(`.o_kanban_card[data-id="${id}"]`);
            if (card !== null) {
                card.scrollIntoView({ block: "nearest", inline: "nearest" });
                card.focus();
                return;
            }
            await new Promise((resolve) => setTimeout(resolve, 50));
        }
    }

    /**
     * Change a field of a card's record from the card: written at once, shown at once, its
     * column read again for what depends on it.
     */
    readonly quickEdit = (record: Values, column: Column, value: unknown): void => {
        const id = record.id as number;
        const name = column.name;
        const type = column.field.type;
        const written = type === "ref" ? groupValue(value) : type === "refs" ? idsOfValue(value) : value;
        const changed = { ...record, [name]: type === "refs" ? namedOfValue(value) : value };
        this.laneCards = new Map(
            [...this.laneCards].map(([lane, cards]) => [lane, cards.map((card) => (card.id === id ? changed : card))]),
        );
        if (this.quick !== null && this.quick.record.id === id) {
            this.quick = { ...this.quick, record: changed };
        }
        const lane = this.groupField === null ? "" : keyOf(record[this.groupField]);
        void this.orm
            .write(this.props.resModel, [id], { [name]: written })
            .then(() => this.rereadLanes(name === this.groupField ? undefined : [lane]))
            .catch((error) => {
                this.notifications.add("danger", error instanceof Error ? error.message : String(error));
                this.rereadLanes([lane]);
            });
    };

    /** Open the editor of a card's field under what was clicked. */
    readonly openQuickEdit = (record: Values, column: Column, event: MouseEvent): void => {
        const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const left = Math.min(box.left, window.innerWidth - 300);
        this.quick = { record, column, place: `top: ${box.bottom + 6}px; left: ${Math.max(8, left)}px` };
    };

    readonly closeQuickEdit = (): void => {
        this.quick = null;
    };

    /** The editor of the field being changed: its type's, a list of records as tags. */
    quickWidget(column: Column): ComponentClass {
        const plain = { ...column, widget: undefined };
        return column.field.type === "refs" ? widgetFor(plain, "tags") : widgetFor(plain);
    }

    /** The attributes the editor takes: no record created from it. */
    quickAttrs(column: Column): Record<string, string> {
        return { ...column.attrs, no_create: "1" };
    }

    readonly quickChange = (value: unknown): void => {
        if (this.quick !== null) {
            this.quickEdit(this.quick.record, this.quick.column, value);
        }
    };

    private reload(): void {
        this.rereadLanes();
        refresh(() => this.groups);
    }

    /** Count a card more or less in a column, without counting them again. */
    private bump(lane: string, by: number): void {
        const groups = this.groups ?? null;
        const deltas = this.deltasOf === groups ? new Map(this.countDeltas) : new Map<string, number>();
        deltas.set(lane, (deltas.get(lane) ?? 0) + by);
        this.deltasOf = groups;
        this.countDeltas = deltas;
    }
}

viewKinds.add("kanban", KanbanView);
