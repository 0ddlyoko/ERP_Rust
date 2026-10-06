import { computed, effect, load, loading, props, resource, state, t } from "trame";
import { decorationNames, decorationOf, evaluate } from "@web/core/expression";
import { listMemory } from "@web/core/list_memory";
import type { ActionDescription } from "@web/core/menus";
import type { Values } from "@web/core/orm";
import { periodLabel } from "@web/views/list/list_view";
import { type Column, View, viewProps } from "@web/views/view";
import { companionFields } from "@web/views/widgets/decimal_widget";
import { type CompiledCard, compileCard } from "./card_compiler";

/** How many records the strip shows at most. */
const LIMIT = 80;
/** How long the pointer rests on a record before its preview shows, in milliseconds. */
const PREVIEW_AFTER = 300;
/** How wide the open strip may be made, in pixels. */
const WIDTH_MIN = 260;
const WIDTH_MAX = 560;
const WIDTH_DEFAULT = 360;
/** Below this width the strip keeps folded; below the next, it is not shown at all. */
const FOLDS_BELOW = "(max-width: 1100px)";

/** Records of the strip sharing a group's value, under its heading. */
interface StripGroup {
    key: string;
    label: string;
    /** The decoration its value is coloured with, as the list colours it. */
    decoration: string | null;
    records: Values[];
}

/** What the browser keeps of a strip, per action: folded or open, and how wide. */
interface StripLayout {
    folded: boolean;
    width: number;
}

function readLayout(action: string | null): StripLayout {
    try {
        const kept = JSON.parse(localStorage.getItem(`o_strip:${action}`) ?? "null") as Partial<StripLayout> | null;
        return { folded: kept?.folded === true, width: Number(kept?.width) || WIDTH_DEFAULT };
    } catch {
        return { folded: false, width: WIDTH_DEFAULT };
    }
}

/** The first day of the period a day falls in, as `read_group` keys a date gathered by period. */
function periodStart(day: string, period: string): string {
    const [year, month, date] = day.slice(0, 10).split("-").map(Number);
    const start = new Date(Date.UTC(year, month - 1, date));
    if (period === "week") {
        start.setUTCDate(start.getUTCDate() - ((start.getUTCDay() + 6) % 7));
    } else if (period === "month") {
        start.setUTCDate(1);
    } else if (period === "quarter") {
        start.setUTCMonth(Math.floor((month - 1) / 3) * 3, 1);
    } else if (period === "year") {
        start.setUTCMonth(0, 1);
    }
    return start.toISOString().slice(0, 10);
}

/**
 * The records of the list a form was opened from, beside it: open as cards — the list's
 * `<compact>` — or folded into a narrow strip — its `<folded>` — each previewed, after a moment
 * under the pointer, as its `<preview>` says. A list saying none of them shows its records' names,
 * with their contact.
 *
 * It follows the list as the user left it: the same search, order and groups, each heading
 * coloured as the list colours the value. Everything a card or a preview shows is read at once,
 * with the records: resting on one asks the server nothing.
 *
 * Folded or open, and how wide, is kept per action by the browser; on a narrow screen it keeps
 * folded. Folded, it opens over the form while the pointer rests on its top.
 */
export class RecordStrip extends View {
    static template = "web.RecordStrip";

    override props = props({
        ...viewProps,
        /** The action whose records these are. */
        action: t.any<ActionDescription>(),
        /** The record the form shows. */
        selected: t.number().orNull().default(null),
    });

    override get kind(): string {
        return "list";
    }

    @state accessor layout: StripLayout = readLayout(this.router.route.action);
    @state accessor narrowScreen = window.matchMedia(FOLDS_BELOW).matches;
    /** Folded, opened over the form while the pointer is on it. */
    @state accessor peeking = false;
    /** Just folded under the pointer: it opens again only once the pointer left and came back. */
    private justFolded = false;
    @state accessor preview: { record: Values; top: number; left: number; above: boolean } | null = null;

    private previewTimer: ReturnType<typeof setTimeout> | undefined;

    /** The strip's element, set by its template. */
    element: HTMLElement | null = null;

    get folded(): boolean {
        return this.layout.folded || this.narrowScreen;
    }

    /** Folded and not opened over the form: the narrow strip shows. */
    get narrow(): boolean {
        return this.folded && !this.peeking;
    }

    get panelStyle(): string {
        return this.narrow ? "" : `width: ${this.layout.width}px`;
    }

    @effect followScreen(): () => void {
        const query = window.matchMedia(FOLDS_BELOW);
        const follow = (): void => {
            this.narrowScreen = query.matches;
        };
        query.addEventListener("change", follow);
        return () => query.removeEventListener("change", follow);
    }

    private keep(layout: StripLayout): void {
        this.layout = layout;
        try {
            localStorage.setItem(`o_strip:${this.router.route.action}`, JSON.stringify(layout));
        } catch {
            // Kept for this page only when the browser keeps nothing.
        }
    }

    fold(): void {
        this.peeking = false;
        this.justFolded = true;
        this.keep({ ...this.layout, folded: true });
    }

    /** Open the folded strip over the form, the pointer resting on its top. */
    peek(): void {
        if (!this.justFolded) {
            this.peeking = true;
        }
    }

    /** The pointer left the strip: it closes, and opens again the next time it comes. */
    leave(): void {
        this.justFolded = false;
        this.peeking = false;
        this.hidePreview();
    }

    unfold(): void {
        this.peeking = false;
        this.keep({ ...this.layout, folded: false });
    }

    /** Size the open strip by dragging its edge. */
    resize(event: PointerEvent): void {
        const startX = event.clientX;
        const startWidth = this.layout.width;
        const move = (moved: PointerEvent): void => {
            const width = Math.max(WIDTH_MIN, Math.min(WIDTH_MAX, startWidth + moved.clientX - startX));
            this.layout = { ...this.layout, width };
        };
        const up = (): void => {
            window.removeEventListener("pointermove", move);
            window.removeEventListener("pointerup", up);
            document.body.classList.remove("o_resizing");
            this.keep(this.layout);
        };
        document.body.classList.add("o_resizing");
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
    }

    /** The list as the user left it, if they came from it. */
    get memory() {
        return listMemory(this.router.route.action);
    }

    /** What the records are gathered by: the field, and the period of a date. */
    get grouping(): { field: string; period: string | null } | null {
        const groupBy = this.memory?.groupBy ?? null;
        if (groupBy === null) {
            return null;
        }
        const [field, period] = groupBy.split(":");
        return { field, period: period ?? null };
    }

    /** The column of the list showing a field, whose decorations colour its value. */
    private listColumn(name: string): Column | undefined {
        return this.columns.find((column) => column.name === name);
    }

    /** The column the strip colours its records by: the field they are gathered by, else the first badge. */
    @computed get colourColumn(): Column | undefined {
        const grouping = this.grouping;
        if (grouping !== null) {
            return this.listColumn(grouping.field);
        }
        return this.columns.find((column) => column.widget === "badge");
    }

    /** A card the list declares, or the one every list has: the records' names and their contact. */
    private card(tag: "compact" | "folded" | "preview"): CompiledCard | null {
        const root = this.archRoot;
        const fields = this.fields;
        if (root === undefined || fields === undefined) {
            return null;
        }
        const declared = Array.from(root.children).find((element) => element.tagName === tag);
        const element = declared ?? this.defaultCard(tag);
        return compileCard(element, (child) => this.columnOf(child, fields));
    }

    private defaultCard(tag: string): Element {
        const fields = this.fields ?? {};
        const title = Object.keys(fields).find((name) => fields[name].name_field) ?? ("name" in fields ? "name" : "id");
        const contact = Object.keys(fields).find(
            (name) => name !== title && fields[name].type === "ref" && fields[name].relation === "contact",
        );
        const avatar = contact === undefined ? "" : `<field name="${contact}" widget="avatar"/>`;
        const below = contact === undefined || tag === "folded" ? "" : `<muted><field name="${contact}"/></muted>`;
        const xml = `<${tag}><row>${avatar}<column><title><field name="${title}"/></title>${below}</column></row></${tag}>`;
        return new DOMParser().parseFromString(xml, "text/xml").documentElement;
    }

    @computed get compact(): CompiledCard | null {
        return this.card("compact");
    }

    @computed get foldedCard(): CompiledCard | null {
        return this.card("folded");
    }

    @computed get previewCard(): CompiledCard | null {
        return this.card("preview");
    }

    /** Every field the cards, the colours and the groups read: read once, with the records. */
    @computed get readNames(): string[] {
        const fields = this.fields ?? {};
        const cards = [this.compact, this.foldedCard, this.previewCard].flatMap((card) => card?.columns ?? []);
        const colour = this.colourColumn;
        const names = [
            ...cards.map((column) => column.name),
            ...companionFields(cards, fields),
            ...(colour === undefined ? [] : [colour.name, ...decorationNames(colour.attrs)]),
            ...(this.grouping === null ? [] : [this.grouping.field]),
        ];
        return [...new Set(names)].filter((name) => name in fields);
    }

    @resource accessor records: Values[] = load(
        () => {
            const memory = this.memory;
            const grouping = this.grouping;
            const order = memory?.order ?? [];
            return {
                model: this.props.resModel,
                domain: memory?.domain ?? this.props.domain,
                fields: loading(() => this.fields) || loading(() => this.arch) ? null : this.readNames,
                order: grouping === null ? (order.length ? order : undefined) : [grouping.field, ...order],
            };
        },
        ({ model, domain, fields, order }) =>
            fields === null
                ? Promise.resolve([])
                : this.orm.searchRead(model, [...domain], fields, { limit: LIMIT, order, names: true }),
    );

    /**
     * The records under their groups' headings — a selection's in the order it lists its values,
     * as the list shows them; one group without a heading when not gathered.
     */
    @computed get groups(): StripGroup[] {
        const records = loading(() => this.records) ? [] : (this.records ?? []);
        const grouping = this.grouping;
        const colour = this.colourColumn;
        const decorate = (record: Values): string | null => (colour === undefined ? null : decorationOf(colour.attrs, record));
        if (grouping === null) {
            return [{ key: "all", label: "", decoration: null, records }];
        }
        const groups = new Map<string, StripGroup>();
        for (const record of records) {
            const { key, label } = this.groupOf(record, grouping.field, grouping.period);
            const group = groups.get(key);
            if (group !== undefined) {
                group.records.push(record);
            } else {
                groups.set(key, { key, label, decoration: decorate(record), records: [record] });
            }
        }
        const order = (this.fields?.[grouping.field]?.values ?? []).map(([key]) => key);
        const rank = (group: StripGroup): number => {
            const at = order.indexOf(group.key);
            return at < 0 ? order.length : at;
        };
        return [...groups.values()].sort((left, right) => rank(left) - rank(right));
    }

    private groupOf(record: Values, name: string, period: string | null): { key: string; label: string } {
        const value = record[name];
        const field = this.fields?.[name];
        if (value === null || value === undefined || value === false || value === "") {
            return { key: "none", label: "None" };
        }
        if (Array.isArray(value)) {
            return { key: String(value[0]), label: String(value[1] ?? `#${value[0]}`) };
        }
        if (field?.type === "selection") {
            return { key: String(value), label: field.values?.find(([key]) => key === value)?.[1] ?? String(value) };
        }
        if (field?.type === "bool") {
            return { key: String(value), label: value ? "Yes" : "No" };
        }
        if ((field?.type === "date" || field?.type === "datetime") && typeof value === "string") {
            const start = periodStart(value, period ?? "day");
            return { key: start, label: periodLabel(start, period ?? "day") };
        }
        return { key: String(value), label: String(value) };
    }

    get count(): number {
        return loading(() => this.records) ? 0 : (this.records?.length ?? 0);
    }

    isSelected(record: Values): boolean {
        return record.id === this.props.selected;
    }

    /** The colour of a record's edge: its value's, as the list colours it. */
    edgeClass(record: Values): string {
        const colour = this.colourColumn;
        const decoration = colour === undefined ? null : decorationOf(colour.attrs, record);
        return [
            "o_strip_item",
            this.isSelected(record) ? "selected" : "",
            decoration === null ? "" : `o_strip_edge_${decoration}`,
        ]
            .filter(Boolean)
            .join(" ");
    }

    /** Whether a condition of a card holds for a record. */
    holds(record: Values, expression: string): boolean {
        return !!evaluate(expression, record);
    }

    open(record: Values): void {
        this.hidePreview();
        if (!this.isSelected(record)) {
            void this.router.go({ ...this.router.route, id: record.id as number });
        }
    }

    create(): void {
        void this.router.go({ ...this.router.route, id: null });
    }

    /** Show a record's preview once the pointer rested on it, beside it — above when low on the screen. */
    hover(record: Values, event: MouseEvent): void {
        clearTimeout(this.previewTimer);
        const target = event.currentTarget as HTMLElement;
        this.previewTimer = setTimeout(() => {
            const box = target.getBoundingClientRect();
            const above = box.top > window.innerHeight - 240;
            this.preview = {
                record,
                left: box.right + 14,
                top: above ? window.innerHeight - box.bottom : box.top,
                above,
            };
        }, PREVIEW_AFTER);
    }

    hidePreview(): void {
        clearTimeout(this.previewTimer);
        this.preview = null;
    }

    get previewStyle(): string {
        const preview = this.preview;
        if (preview === null) {
            return "";
        }
        return `left: ${preview.left}px; ${preview.above ? "bottom" : "top"}: ${preview.top}px`;
    }

    /** The record open shows in the strip, scrolled to if it has to be. */
    @effect showSelected(): void {
        if (loading(() => this.records) || this.records === undefined || this.props.selected === null) {
            return;
        }
        queueMicrotask(() => this.element?.querySelector(".o_strip_item.selected")?.scrollIntoView({ block: "nearest" }));
    }
}
