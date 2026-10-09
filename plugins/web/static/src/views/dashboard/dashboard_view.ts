import { computed, load, props, resource, state } from "trame";
import { openWith } from "@web/core/list_memory";
import type { Domain } from "@web/core/orm";
import { listKey } from "@web/core/router";
import { periodLabel } from "@web/views/list/list_view";
import { View, viewKinds, viewProps } from "@web/views/view";
import {
    dateOf,
    dayBefore,
    dayOf,
    type Period,
    PERIODS,
    type Range,
    rangeOf,
    readTile,
    type Scope,
    stepped,
    type Tile,
} from "./dashboard_model";
import { type ChartView, DashboardChart, DashboardMetric, DashboardRecords, type TileHost } from "./dashboard_tiles";

/** The period a dashboard was last shown for, per action, as the browser keeps it. */
function readPeriod(action: string | null): Period | null {
    try {
        const kept = localStorage.getItem(`o_dashboard_period.${action}`);
        return PERIODS.some((period) => period.key === kept) ? (kept as Period) : null;
    } catch {
        return null;
    }
}

/**
 * An app's dashboard: its figures for a month, quarter or year — today's, or one gone to before
 * or after — its charts and the records to see to, each a tile of the `<dashboard>` arch, opening
 * the records it stands for.
 *
 * `<dashboard date="date_order" filter_by="user" currency_field="currency" period="month">`: the
 * date narrowed to the period chosen, a field to narrow every tile to one of its records, the
 * field holding the records' currency, and the period it opens on. Its tiles:
 *
 * - `<metric string="Confirmed" sum="amount_untaxed" domain="…" compare="1"/>`: a figure — a sum,
 *   an `average`, else a count — beside that of the period before, or with `counted="orders"`
 *   how many records it adds up;
 * - `<chart type="columns|bars|ranking" group_by="partner" sum="…" limit="5"/>`: columns by
 *   period, the last ones up to today's (`last="6"`); bars by value; the largest values first;
 * - `<records string="To follow up" domain="…" limit="5" order="date_order"><card>…</card></records>`:
 *   the first records, as cards, and how many there are.
 *
 * Any of them may be of another `model`, dated by its own `date`, or count `all_time="1"`;
 * `widget="monetary"` or `"hours"` writes its figures; `action` is the list a click opens.
 */
export class DashboardView extends View implements TileHost {
    static template = "web.DashboardView";
    static components = { DashboardChart, DashboardMetric, DashboardRecords };
    override props = props({ ...viewProps });

    get kind(): string {
        return "dashboard";
    }

    readonly periods = PERIODS;

    @state accessor chosen: Period | null = readPeriod(this.router.route.action);
    /** A day of the period shown: today until the user goes to another month, quarter or year. */
    @state accessor day: string = dayOf(new Date());
    /** The record of `filter_by` the tiles are narrowed to. */
    @state accessor filterId: number | null = null;

    get period(): Period {
        const declared = this.archRoot?.getAttribute("period");
        return this.chosen ?? (PERIODS.some((period) => period.key === declared) ? (declared as Period) : "month");
    }

    /** The days shown: of the month, quarter or year holding the day chosen. */
    get range(): Range | null {
        return rangeOf(this.period, 0, dateOf(this.day));
    }

    /** The period shown, as people name it: `October 2026`. */
    get rangeText(): string {
        const range = this.range;
        return range === null ? "Since the start" : periodLabel(range.start, this.period);
    }

    /** Whether the period shown is the one holding today. */
    get isCurrent(): boolean {
        const range = this.range;
        const today = dayOf(new Date());
        return range === null || (range.start <= today && today < range.end);
    }

    /** Go to the period before, `-1`, or after, `1`. */
    step(by: number): void {
        this.day = stepped(this.day, this.period, by);
    }

    /** Back to the period holding today. */
    backToToday(): void {
        this.day = dayOf(new Date());
    }

    pick(period: Period): void {
        this.chosen = period;
        try {
            localStorage.setItem(`o_dashboard_period.${this.router.route.action}`, period);
        } catch {
            // The browser keeps nothing: the dashboard opens on its own period next time.
        }
    }

    /** The field every tile may be narrowed by. */
    get filterField(): string | null {
        return this.archRoot?.getAttribute("filter_by") ?? null;
    }

    get filterLabel(): string {
        const field = this.filterField;
        return field === null ? "" : (this.fields?.[field]?.label ?? field);
    }

    /** The records the dashboard may be narrowed to: those its records point to, by name. */
    @resource accessor filterChoices: [number, string][] = load(
        () => (this.filterField === null ? null : { model: this.props.resModel, field: this.filterField }),
        async (asked) => {
            if (asked === null) {
                return [];
            }
            const groups = await this.orm.readGroup(asked.model, [], asked.field);
            return groups
                .map((group) => group.value)
                .filter((value): value is [number, string] => Array.isArray(value) && typeof value[0] === "number")
                .map(([id, name]) => [id, name ?? String(id)] as [number, string])
                .sort((left, right) => left[1].localeCompare(right[1]));
        },
    );

    chooseFilter(event: Event): void {
        const value = Number((event.target as HTMLSelectElement).value);
        this.filterId = Number.isInteger(value) && value > 0 ? value : null;
    }

    /** The currency every amount is in, when the records share one: `EUR`. */
    @resource accessor currency: string | null = load(
        () => {
            const field = this.archRoot?.getAttribute("currency_field") ?? null;
            return field === null ? null : { model: this.props.resModel, field };
        },
        async (asked) => {
            if (asked === null) {
                return null;
            }
            const groups = await this.orm.readGroup(asked.model, [], asked.field);
            const codes = groups.map((group) => (Array.isArray(group.value) ? group.value[1] : null));
            return codes.length === 1 && typeof codes[0] === "string" && /^[A-Z]{3}$/.test(codes[0]) ? codes[0] : null;
        },
    );

    @computed get scope(): Scope {
        const field = this.filterField;
        const range = this.range;
        const today = dayOf(new Date());
        return {
            period: this.period,
            until: range === null || range.end > today ? today : dayBefore(range.end),
            range,
            previous: rangeOf(this.period, 1, dateOf(this.day)),
            filter: field !== null && this.filterId !== null ? { field, id: this.filterId } : null,
            currency: this.currency ?? null,
        };
    }

    @computed get tiles(): Tile[] {
        const root = this.archRoot;
        if (root === undefined) {
            return [];
        }
        const date = root.getAttribute("date");
        return Array.from(root.children)
            .filter((element) => ["metric", "chart", "records"].includes(element.tagName))
            .map((element, at) => readTile(element, at, this.props.resModel, date));
    }

    get metrics(): Tile[] {
        return this.tiles.filter((tile) => tile.tag === "metric");
    }

    /** The charts and lists of records, laid out side by side as room allows. */
    get panels(): Tile[] {
        return this.tiles.filter((tile) => tile.tag !== "metric");
    }

    panelClass(tile: Tile): string {
        return tile.tag === "chart" && tile.chart === "columns" ? "o_dash_panel o_dash_panel_wide" : "o_dash_panel";
    }

    openList(tile: Tile, domain: Domain, label: string): void {
        if (tile.action === null) {
            return;
        }
        const route = { action: tile.action, view: null, id: null };
        openWith(listKey(route), [{ kind: "domain", label, domain }]);
        void this.router.go(route);
    }

    /** Where the browser keeps how a chart of this dashboard is drawn. */
    private viewKey(tile: Tile): string {
        return `o_dashboard_view.${this.router.route.action}.${tile.key}`;
    }

    viewOf(tile: Tile): ChartView | null {
        try {
            const kept = localStorage.getItem(this.viewKey(tile));
            return kept === "columns" || kept === "bars" || kept === "table" ? kept : null;
        } catch {
            return null;
        }
    }

    keepView(tile: Tile, view: ChartView): void {
        try {
            localStorage.setItem(this.viewKey(tile), view);
        } catch {
            // The browser keeps nothing: the chart is drawn as its arch says next time.
        }
    }

    openRecord(tile: Tile, id: number): void {
        if (tile.action !== null) {
            void this.router.go({ action: tile.action, view: "form", id });
        }
    }
}

viewKinds.add("dashboard", DashboardView);
