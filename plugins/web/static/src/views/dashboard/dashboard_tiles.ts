import { Component, type ComponentClass, computed, inject, load, props, resource, state, t } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { and } from "@web/core/domain";
import { evaluate } from "@web/core/expression";
import { type Fields, Models } from "@web/core/models";
import { type Domain, type Group, Orm, type Values } from "@web/core/orm";
import { type CompiledCard, compileCard } from "@web/views/strip/card_compiler";
import { type Column, columnOf, widgetFor } from "@web/views/view";
import { companionFields } from "@web/views/widgets/decimal_widget";
import { periodLabel } from "@web/views/list/list_view";
import {
    bucketOf,
    bucketsOf,
    comparedBuckets,
    type Condition,
    counting,
    dateOf,
    type Scope,
    scaleTop,
    shortLabel,
    type Tile,
    valueLabel,
    within,
    written,
} from "./dashboard_model";

/** What a tile asks of the dashboard showing it: to open records it shows. */
export interface TileHost {
    /** Open the tile's records meeting these conditions, as a list searched for them, a chip each. */
    openList(tile: Tile, conditions: Condition[]): void;
    /** Open one record of the tile. */
    openRecord(tile: Tile, id: number): void;
    /** How the user last chose to see a chart, if they did. */
    viewOf(tile: Tile): ChartView | null;
    /** Keep how the user chose to see a chart. */
    keepView(tile: Tile, view: ChartView): void;
}

/** How a chart is drawn, whatever its records: columns standing, bars lying, or a table. */
export type ChartView = "columns" | "bars" | "table";

export const CHART_VIEWS: readonly { key: ChartView; label: string; icon: string }[] = [
    { key: "columns", label: "Columns", icon: "M6 20V10M12 20V4M18 20v-7M3 20h18" },
    { key: "bars", label: "Bars", icon: "M4 6h10M4 12h16M4 18h7M4 3v18" },
    { key: "table", label: "Table", icon: "M3 5h18v14H3zM3 10h18M3 15h18M9 5v14" },
];

/** The colours bars are painted in, one after another: each readable beside the next. */
const BAR_COLOURS = ["#4b3fe0", "#0f8a6a", "#c8650f", "#3a86cc", "#8e1f55", "#6e5200", "#24489e", "#0e6b52"];

/** How a value changed since another: `+12%`, `−4%`, nothing when there was none before. */
function changeOf(now: number, before: number | null): string {
    if (before === null || before === 0) {
        return "";
    }
    const change = Math.round(((now - before) / Math.abs(before)) * 100);
    return change > 0 ? `+${change}%` : change < 0 ? `−${-change}%` : "=";
}

/** The amount of a group: the sum of the field asked, its average, or how many records it holds. */
function amountOf(tile: Tile, group: Group | undefined): number {
    if (group === undefined) {
        return 0;
    }
    if (tile.sum !== null) {
        return Number(group.sums[tile.sum] ?? 0);
    }
    if (tile.average !== null) {
        return group.count === 0 ? 0 : Number(group.sums[tile.average] ?? 0) / group.count;
    }
    return group.count;
}

/**
 * What every tile shares: its model's fields, and the records it covers — those of its domain,
 * within the period on its date, narrowed to the dashboard's filter where its model has the field.
 * What it reads is asked again as its records change.
 */
abstract class DashboardTile extends Component {
    props = props({
        tile: t.any<Tile>(),
        scope: t.any<Scope>(),
        host: t.any<TileHost>(),
        /** The tile's model, as a card's widgets read it. */
        resModel: t.string(),
    });

    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;

    get tile(): Tile {
        return this.props.tile as Tile;
    }

    get scope(): Scope {
        return this.props.scope as Scope;
    }

    @resource accessor fields: Fields = load(
        () => this.props.resModel,
        (model) => this.models.fields(model),
    );

    /** The dashboard's filter, on a model that has its field. */
    get filtered(): Condition[] {
        const filter = this.scope.filter;
        if (filter === null || this.fields?.[filter.field] === undefined) {
            return [];
        }
        return [{ label: `${filter.label}: ${filter.name}`, domain: [[filter.field, "=", filter.id]] }];
    }

    /** The tile's own domain, named after it. */
    get own(): Condition[] {
        return this.tile.domain.length ? [{ label: this.tile.label, domain: this.tile.domain }] : [];
    }

    /** What the tile's records meet in the period chosen, or the one before: each condition named. */
    conditionsIn(back: 0 | 1): Condition[] {
        const range = back === 0 ? this.scope.range : this.scope.previous;
        const period =
            this.tile.date === null || range === null
                ? []
                : [{ label: periodLabel(range.start, this.scope.period), domain: within(this.tile.date, range) }];
        return [...this.own, ...period, ...this.filtered];
    }

    /** The tile's records in the period chosen, or the one before. */
    domainIn(back: 0 | 1): Domain {
        return and(this.conditionsIn(back).map((condition) => condition.domain));
    }

    /** The fields a group adds up. */
    get sums(): string[] {
        return [this.tile.sum, this.tile.average].filter((name): name is string => name !== null);
    }

    /** What the tile reads, as text: the same text, the same answer. */
    keyOf(asked: Record<string, unknown>): string {
        return JSON.stringify({ ...asked, model: this.props.resModel, version: this.orm.versionOf(this.props.resModel) });
    }

    written(value: number, compact = false): string {
        return written(value, this.tile.widget, this.scope.currency, compact);
    }
}

/** A figure of a period: what it adds up to, and how it compares with the period before. */
export class DashboardMetric extends DashboardTile {
    static template = "web.DashboardMetric";

    @resource accessor figures: { now: Group | undefined; before: Group | undefined } | null = load(
        () => this.keyOf({ now: this.domainIn(0), before: this.tile.compare && this.scope.previous ? this.domainIn(1) : null }),
        async (key) => {
            const { model, now, before } = JSON.parse(key) as { model: string; now: Domain; before: Domain | null };
            try {
                const [current, previous] = await Promise.all([
                    this.orm.readGroup(model, now, null, this.sums),
                    before === null ? Promise.resolve([]) : this.orm.readGroup(model, before, null, this.sums),
                ]);
                return { now: current[0], before: before === null ? undefined : previous[0] };
            } catch {
                return null;
            }
        },
    );

    get value(): string {
        return this.figures === null ? "—" : this.written(amountOf(this.tile, this.figures.now));
    }

    /** How far the period got towards the metric's target: `76% of €80K`, and whether it is reached. */
    get progress(): { share: number; text: string; reached: boolean } | null {
        const target = this.tile.target;
        if (target === null || target === 0 || this.figures === null) {
            return null;
        }
        const ratio = amountOf(this.tile, this.figures.now) / target;
        return {
            share: Math.max(0, Math.min(100, ratio * 100)),
            text: `${Math.round(ratio * 100)}% of ${this.written(target, true)}`,
            reached: ratio >= 1,
        };
    }

    /** `▲ 83% vs September 2026 (33,500 €)`, or what it counts: `8 orders`. */
    get caption(): { text: string; tone: string } | null {
        const figures = this.figures;
        if (figures === null) {
            return null;
        }
        const previous = this.scope.previous;
        if (this.tile.compare && previous !== null && figures.before !== undefined) {
            const now = amountOf(this.tile, figures.now);
            const before = amountOf(this.tile, figures.before);
            const label = periodLabel(previous.start, this.scope.period);
            if (before === 0) {
                return { text: `None in ${label}`, tone: "" };
            }
            const change = Math.round(((now - before) / Math.abs(before)) * 100);
            const arrow = change > 0 ? "▲" : change < 0 ? "▼" : "=";
            return {
                text: `${arrow} ${Math.abs(change)}% vs ${label} (${this.written(before)})`,
                tone: change > 0 ? "o_dash_up" : change < 0 ? "o_dash_down" : "",
            };
        }
        if (this.tile.counted !== null) {
            return { text: counting(figures.now?.count ?? 0, this.tile.counted), tone: "" };
        }
        return null;
    }

    get opens(): boolean {
        return this.tile.action !== null;
    }

    open(): void {
        if (this.opens) {
            this.props.host.openList(this.tile, this.conditionsIn(0));
        }
    }
}

/** A bar, a column or a line of a ranking: a value, how long it is drawn, the records it stands for. */
interface Bar {
    key: string;
    label: string;
    /** The longer label of a column: its whole period. */
    full: string;
    value: number;
    text: string;
    count: number;
    countText: string;
    /** The value of the period compared with, when the chart compares. */
    before: number | null;
    beforeText: string;
    beforeShare: number;
    /** How it changed since: `+12%`, `−4%`. */
    change: string;
    /** The records it stands for, each condition named. */
    conditions: Condition[];
    /** Its length against the longest, as a percentage. */
    share: number;
    style: string;
    avatar: string;
    current: boolean;
}

/**
 * A chart of the records gathered by a field. By a date, a value by period: the last periods up
 * to the one the dashboard shows. By anything else, a value per group within that period: in the
 * order of a selection's choices, or the largest first — with their initials for a `ranking`, as
 * of clients or products. Drawn as the arch's `type` says — `columns` standing, else bars lying —
 * until the user picks another way, a table among them. A bar opens the records it stands for.
 */
export class DashboardChart extends DashboardTile {
    static template = "web.DashboardChart";

    readonly views = CHART_VIEWS;

    @state accessor chosen: ChartView | null = null;

    /** How the chart is drawn: as the user chose, else as the arch says. */
    get shown(): ChartView {
        return this.chosen ?? this.props.host.viewOf(this.tile) ?? (this.tile.chart === "columns" ? "columns" : "bars");
    }

    show(view: ChartView): void {
        this.chosen = view;
        this.props.host.keepView(this.tile, view);
    }

    /** The field the records are gathered by, and the period of a date. */
    get grouping(): { field: string; period: string | null } {
        const [field, period] = (this.tile.groupBy ?? "").split(":");
        return { field, period: period ?? (this.timed ? "month" : null) };
    }

    /** Whether the records are gathered by a date: a value by period. */
    get timed(): boolean {
        const type = this.fields?.[(this.tile.groupBy ?? "").split(":")[0]]?.type;
        return type === "date" || type === "datetime";
    }

    /** The domain the chart reads: its periods for a date, the dashboard's period otherwise. */
    get asked(): { domain: Domain; groupBy: string } {
        const { field, period } = this.grouping;
        const groupBy = period === null ? field : `${field}:${period}`;
        if (!this.timed) {
            return { domain: this.domainIn(0), groupBy };
        }
        const buckets = bucketsOf(period ?? "month", this.tile.last, dateOf(this.scope.until));
        const span = { start: buckets[0].start, end: buckets[buckets.length - 1].end };
        return { domain: and([this.tile.domain, within(field, span), ...this.filtered.map((condition) => condition.domain)]), groupBy };
    }

    @resource accessor groups: Group[] | null = load(
        () => (this.fields === undefined ? null : this.keyOf(this.asked)),
        async (key) => {
            if (key === null) {
                return null;
            }
            const { model, domain, groupBy } = JSON.parse(key) as { model: string; domain: Domain; groupBy: string };
            try {
                return await this.orm.readGroup(model, domain, groupBy, this.sums);
            } catch {
                return null;
            }
        },
    );

    /** The domain the chart compares with: the periods a year before, or the period before. */
    get comparedAsked(): { domain: Domain; groupBy: string } | null {
        if (!this.tile.compare) {
            return null;
        }
        const { field, period } = this.grouping;
        const groupBy = period === null ? field : `${field}:${period}`;
        if (!this.timed) {
            return this.scope.previous === null ? null : { domain: this.domainIn(1), groupBy };
        }
        const buckets = comparedBuckets(period ?? "month", this.tile.last, dateOf(this.scope.until));
        const span = { start: buckets[0].start, end: buckets[buckets.length - 1].end };
        return { domain: and([this.tile.domain, within(field, span), ...this.filtered.map((condition) => condition.domain)]), groupBy };
    }

    @resource accessor comparedGroups: Group[] | null = load(
        () => {
            const asked = this.fields === undefined ? null : this.comparedAsked;
            return asked === null ? null : this.keyOf(asked);
        },
        async (key) => {
            if (key === null) {
                return null;
            }
            const { model, domain, groupBy } = JSON.parse(key) as { model: string; domain: Domain; groupBy: string };
            try {
                return await this.orm.readGroup(model, domain, groupBy, this.sums);
            } catch {
                return null;
            }
        },
    );

    /** What the chart compares with, by bar: a period by its place among the columns, else a group by its value. */
    private get compared(): Map<string, number> | null {
        const groups = this.comparedGroups;
        if (groups === null || groups === undefined) {
            return null;
        }
        const { period } = this.grouping;
        if (!this.timed) {
            return new Map(groups.map((group) => [JSON.stringify(group.value), amountOf(this.tile, group)]));
        }
        const buckets = comparedBuckets(period ?? "month", this.tile.last, dateOf(this.scope.until));
        const byStart = new Map(groups.filter((group) => typeof group.value === "string").map((group) => [bucketOf(group.value as string, period ?? "month"), group]));
        return new Map(buckets.map((bucket, at) => [String(at), amountOf(this.tile, byStart.get(bucket.start))]));
    }

    /** What the bars are compared with, as the chart's note says it. */
    get comparedWith(): string {
        if (!this.tile.compare) {
            return "";
        }
        const period = this.grouping.period ?? "month";
        if (this.timed) {
            return period === "month" || period === "quarter" || period === "year" ? "vs a year before" : "vs the periods before";
        }
        const previous = this.scope.previous;
        return previous === null ? "" : `vs ${periodLabel(previous.start, this.scope.period)}`;
    }

    @computed get bars(): Bar[] {
        const groups = this.groups ?? [];
        const { field, period } = this.grouping;
        let bars: Omit<Bar, "share" | "style" | "countText" | "before" | "beforeText" | "beforeShare" | "change">[];
        if (this.timed) {
            const byStart = new Map(groups.filter((group) => typeof group.value === "string").map((group) => [bucketOf(group.value as string, period ?? "month"), group]));
            const buckets = bucketsOf(period ?? "month", this.tile.last, dateOf(this.scope.until));
            bars = buckets.map((bucket, at) => {
                const group = byStart.get(bucket.start);
                const value = amountOf(this.tile, group);
                return {
                    key: bucket.start,
                    label: shortLabel(bucket.start, period ?? "month"),
                    full: periodLabel(bucket.start, period ?? "month"),
                    value,
                    text: this.written(value),
                    count: group?.count ?? 0,
                    conditions: [...this.own, { label: periodLabel(bucket.start, period ?? "month"), domain: within(field, bucket) }, ...this.filtered],
                    avatar: "",
                    current: at === buckets.length - 1,
                };
            });
        } else {
            const description = this.fields?.[field];
            bars = groups.map((group) => {
                const label = valueLabel(group.value, description);
                const value = amountOf(this.tile, group);
                return {
                    key: JSON.stringify(group.value),
                    label,
                    full: label,
                    value,
                    text: this.written(value),
                    count: group.count,
                    conditions: [...this.conditionsIn(0), { label: `${this.groupLabel}: ${label}`, domain: group.domain }],
                    avatar: avatarStyleOf(label),
                    current: false,
                };
            });
            const ordered = description?.type === "selection" && this.tile.chart !== "ranking";
            if (ordered) {
                const keys = (description.values ?? []).map(([key]) => key);
                const at = (bar: (typeof bars)[number]): number => keys.indexOf(JSON.parse(bar.key) as string);
                bars.sort((left, right) => at(left) - at(right));
            } else {
                bars.sort((left, right) => right.value - left.value);
            }
            bars = bars.filter((bar) => bar.value !== 0 || bar.count !== 0).slice(0, this.tile.limit);
        }
        const compared = this.compared;
        const beforeOf = (bar: (typeof bars)[number], at: number): number | null =>
            compared === null ? null : (compared.get(this.timed ? String(at) : bar.key) ?? 0);
        const largest = Math.max(0, ...bars.flatMap((bar, at) => [Math.abs(bar.value), Math.abs(beforeOf(bar, at) ?? 0)]));
        const top = this.shown === "columns" ? this.top : largest || 1;
        const plain = this.timed || this.tile.chart === "ranking";
        return bars.map((bar, at) => ({
            ...bar,
            before: beforeOf(bar, at),
            beforeText: beforeOf(bar, at) === null ? "" : this.written(beforeOf(bar, at) ?? 0),
            beforeShare: Math.max(0, Math.min(100, ((beforeOf(bar, at) ?? 0) / top) * 100)),
            change: changeOf(bar.value, beforeOf(bar, at)),
            countText: bar.count.toLocaleString(),
            share: Math.max(0, Math.min(100, (bar.value / top) * 100)),
            style: `--o-dash-bar: ${plain ? "var(--o-accent)" : BAR_COLOURS[at % BAR_COLOURS.length]}; --o-dash-at: ${at}`,
        }));
    }

    /** Where the scale of columns ends: a round number over the largest, with room for its label; a count's in thirds. */
    get top(): number {
        const groups = [...(this.groups ?? []), ...(this.comparedGroups ?? [])];
        const largest = Math.max(0, ...groups.map((group) => amountOf(this.tile, group))) * 1.15;
        return this.countShown ? scaleTop(largest) : Math.max(3, Math.ceil(largest / 3) * 3);
    }

    /** The scale beside columns, from the top down to zero. */
    get ticks(): string[] {
        const top = this.top;
        return [1, 2 / 3, 1 / 3, 0].map((part) => (part === 0 ? "0" : this.written(top * part, true)));
    }

    /** What the groups are: the label of the field gathering the records. */
    get groupLabel(): string {
        return this.fields?.[this.grouping.field]?.label ?? this.grouping.field;
    }

    /** What the figures are: the field added up, its average, or a count of records. */
    get valueLabel(): string {
        const field = this.tile.sum ?? this.tile.average;
        const label = field === null ? "Records" : (this.fields?.[field]?.label ?? field);
        return this.tile.average !== null && this.tile.sum === null ? `Average ${label.toLowerCase()}` : label;
    }

    /** Whether a bar says how many records it stands for: not when that is its value already. */
    get countShown(): boolean {
        return this.tile.sum !== null || this.tile.average !== null;
    }

    get empty(): boolean {
        return this.groups !== null && this.bars.every((bar) => bar.count === 0);
    }

    /** What the chart covers: its first period to its last, or the dashboard's period. */
    get note(): string {
        if (this.timed) {
            const bars = this.bars;
            return bars.length ? `${bars[0].full} → ${bars[bars.length - 1].full}` : "";
        }
        const range = this.scope.range;
        return this.tile.date === null || range === null ? "All time" : periodLabel(range.start, this.scope.period);
    }

    initials(label: string): string {
        return initialsOf(label);
    }

    open(bar: Bar): void {
        if (this.tile.action !== null) {
            this.props.host.openList(this.tile, bar.conditions);
        }
    }
}

/** Some records as cards, as the tile's `<card>` lays them out: the first few, and how many there are. */
export class DashboardRecords extends DashboardTile {
    static template = "web.DashboardRecords";

    @computed get card(): CompiledCard | null {
        const fields = this.fields;
        const card = Array.from(this.tile.element.children).find((child) => child.tagName === "card");
        if (fields === undefined || card === undefined) {
            return null;
        }
        return compileCard(card, (child) => columnOf(child, fields, this.props.resModel));
    }

    /** Every field the card shows, with those its widgets read besides. */
    get read(): string[] {
        const columns = this.card?.columns ?? [];
        const names = columns.map((column) => column.name);
        return [...new Set([...names, ...companionFields(columns, this.fields ?? {})])];
    }

    @resource accessor found: { records: Values[]; total: number } | null = load(
        () => (this.card === null ? null : this.keyOf({ domain: this.domainIn(0), read: this.read })),
        async (key) => {
            if (key === null) {
                return null;
            }
            const { model, domain, read } = JSON.parse(key) as { model: string; domain: Domain; read: string[] };
            try {
                const [records, total] = await Promise.all([
                    this.orm.searchRead(model, domain, read, { limit: this.tile.limit, order: this.tile.order, names: true }),
                    this.orm.count(model, domain),
                ]);
                return { records, total };
            } catch {
                return null;
            }
        },
    );

    holds(record: Values, expression: string): boolean {
        return !!evaluate(expression, record);
    }

    widgetFor(column: Column): ComponentClass {
        return widgetFor(column);
    }

    get opens(): boolean {
        return this.tile.action !== null;
    }

    openAll(): void {
        if (this.opens) {
            this.props.host.openList(this.tile, this.conditionsIn(0));
        }
    }

    open(record: Values): void {
        if (this.opens) {
            this.props.host.openRecord(this.tile, record.id as number);
        }
    }
}
