import type { Domain, FieldDescription } from "@web/core/orm";
import { hoursText } from "@web/views/widgets/hours_widget";

/** The time a dashboard's figures cover: this month, quarter or year, or all of it. */
export type Period = "month" | "quarter" | "year" | "all";

export const PERIODS: readonly { key: Period; label: string }[] = [
    { key: "month", label: "Month" },
    { key: "quarter", label: "Quarter" },
    { key: "year", label: "Year" },
    { key: "all", label: "All time" },
];

/** Days from `start`, up to `end` left out, as `YYYY-MM-DD`. */
export interface Range {
    start: string;
    end: string;
}

/** What every tile of a dashboard follows: the period chosen, and the value it is narrowed to. */
export interface Scope {
    period: Period;
    /** The last day columns over time show: today, or the end of a period gone by. */
    until: string;
    /** The days of the period, `null` for all time. */
    range: Range | null;
    /** The days of the period before, to compare with. */
    previous: Range | null;
    /** The field the dashboard narrows its records by, and the record chosen, if one. */
    filter: { field: string; id: number } | null;
    /** The code of the currency amounts are in, when the records share one. */
    currency: string | null;
}

/** A tile of a dashboard, as its `<metric>`, `<chart>` or `<records>` says. */
export interface Tile {
    /** What names it among the dashboard's tiles: its `name`, else its place. */
    key: string;
    tag: "metric" | "chart" | "records";
    label: string;
    model: string;
    domain: Domain;
    /** The date narrowed to the period; `null` for a tile counting all time. */
    date: string | null;
    sum: string | null;
    average: string | null;
    /** How the figures are written: `monetary`, `hours`, or as numbers. */
    widget: string | null;
    /** The action opening its records, from a figure, a bar or a card. */
    action: string | null;
    /** A metric beside the one of the period before. */
    compare: boolean;
    /** What a metric counts, written under it: `order|orders` for `1 order`, `8 orders`. */
    counted: string | null;
    chart: "columns" | "bars" | "ranking";
    groupBy: string | null;
    /** The bars or cards shown at most. */
    limit: number;
    /** The periods a chart of columns goes back. */
    last: number;
    order: string[] | undefined;
    element: Element;
}

/**
 * A tile as its element says, of the dashboard's model unless it names another: everything a tile
 * is lies in its attributes, so that a dashboard can be laid out anew by writing its XML.
 */
export function readTile(element: Element, at: number, model: string, date: string | null): Tile {
    const attr = (name: string): string | null => element.getAttribute(name);
    const own = attr("model") ?? model;
    const chart = attr("type");
    const tag = element.tagName as Tile["tag"];
    return {
        key: attr("name") ?? `${tag}:${at}`,
        tag,
        label: attr("string") ?? "",
        model: own,
        domain: JSON.parse(attr("domain") ?? "[]") as Domain,
        date: attr("all_time") === "1" ? null : (attr("date") ?? (own === model ? date : null)),
        sum: attr("sum"),
        average: attr("average"),
        widget: attr("widget"),
        action: attr("action"),
        compare: attr("compare") === "1",
        counted: attr("counted"),
        chart: chart === "columns" || chart === "ranking" ? chart : "bars",
        groupBy: attr("group_by"),
        limit: Number(attr("limit")) || (tag === "records" || chart === "ranking" ? 5 : 8),
        last: Number(attr("last")) || 6,
        order: attr("order")?.split(",").map((part) => part.trim()) ?? undefined,
        element,
    };
}

/** A day as `YYYY-MM-DD`, in the user's time. */
export function dayOf(date: Date): string {
    const month = String(date.getMonth() + 1).padStart(2, "0");
    return `${date.getFullYear()}-${month}-${String(date.getDate()).padStart(2, "0")}`;
}

/** A period as a column's axis names it, in little room: `Sep`, `Aug 17`, `Q4`, `2026`. */
export function shortLabel(day: string, period: string): string {
    const date = new Date(`${day}T00:00:00`);
    switch (period) {
        case "year":
            return String(date.getFullYear());
        case "quarter":
            return `Q${Math.floor(date.getMonth() / 3) + 1}`;
        case "month":
            return date.toLocaleDateString(undefined, { month: "short" });
        default:
            return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
    }
}

/** `8 orders`, `1 order`, of a noun written `order|orders`, or the same either way. */
export function counting(count: number, noun: string): string {
    const [one, many = one] = noun.split("|");
    return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}

/** The first day of the period `date` falls in. */
function startOf(date: Date, period: string): Date {
    const [year, month, day] = [date.getFullYear(), date.getMonth(), date.getDate()];
    switch (period) {
        case "year":
            return new Date(year, 0, 1);
        case "quarter":
            return new Date(year, month - (month % 3), 1);
        case "month":
            return new Date(year, month, 1);
        case "week":
            return new Date(year, month, day - ((date.getDay() + 6) % 7));
        default:
            return new Date(year, month, day);
    }
}

/** The first day of the period `by` periods after the one starting on `start`. */
function shifted(start: Date, period: string, by: number): Date {
    const [year, month, day] = [start.getFullYear(), start.getMonth(), start.getDate()];
    switch (period) {
        case "year":
            return new Date(year + by, 0, 1);
        case "quarter":
            return new Date(year, month + 3 * by, 1);
        case "month":
            return new Date(year, month + by, 1);
        case "week":
            return new Date(year, month, day + 7 * by);
        default:
            return new Date(year, month, day + by);
    }
}

/** The day `YYYY-MM-DD` as a date, at midnight in the user's time. */
export function dateOf(day: string): Date {
    const [year, month, date] = day.split("-").map(Number);
    return new Date(year, month - 1, date);
}

/** The first day of the period `by` periods after the one holding `day`: the next month, the year before. */
export function stepped(day: string, period: Period, by: number): string {
    return period === "all" ? day : dayOf(shifted(startOf(dateOf(day), period), period, by));
}

/** The day before `day`. */
export function dayBefore(day: string): string {
    const date = dateOf(day);
    return dayOf(new Date(date.getFullYear(), date.getMonth(), date.getDate() - 1));
}

/** The days of the period holding today, or of the one `back` periods before; none for all time. */
export function rangeOf(period: Period, back = 0, today = new Date()): Range | null {
    if (period === "all") {
        return null;
    }
    const start = shifted(startOf(today, period), period, -back);
    return { start: dayOf(start), end: dayOf(shifted(start, period, 1)) };
}

/** The periods a chart of columns shows, oldest first, the one holding today last. */
export function bucketsOf(period: string, count: number, today = new Date()): Range[] {
    const current = startOf(today, period);
    return Array.from({ length: count }, (_, at) => {
        const start = shifted(current, period, at - count + 1);
        return { start: dayOf(start), end: dayOf(shifted(start, period, 1)) };
    });
}

/** The first day of the period holding `day`, as `read_group` keys a date gathered by period. */
export function bucketOf(day: string, period: string): string {
    const [year, month, date] = day.slice(0, 10).split("-").map(Number);
    return dayOf(startOf(new Date(year, month - 1, date), period));
}

/** The records of a date field within a range. */
export function within(field: string | null, range: Range | null): Domain {
    return field === null || range === null
        ? []
        : [
              [field, ">=", range.start],
              [field, "<", range.end],
          ];
}

/** A figure as the tile writes it: an amount in its currency, hours as `h:mm`, or a number. */
export function written(value: number, widget: string | null, currency: string | null, compact = false): string {
    if (widget === "hours") {
        return hoursText(value);
    }
    const options: Intl.NumberFormatOptions = compact
        ? { notation: "compact", maximumFractionDigits: 1 }
        : { minimumFractionDigits: 0, maximumFractionDigits: Math.abs(value) >= 100 ? 0 : 2 };
    if (widget === "monetary" && currency !== null) {
        return value.toLocaleString(undefined, { ...options, style: "currency", currency });
    }
    return value.toLocaleString(undefined, options);
}

/** What a group's value reads as: a record's name, a choice's label, `None` for none. */
export function valueLabel(value: unknown, field: FieldDescription | undefined): string {
    if (value === null || value === undefined || value === false) {
        return field?.type === "bool" ? "No" : "None";
    }
    if (Array.isArray(value)) {
        return String(value[1] ?? value[0]);
    }
    if (field?.type === "selection") {
        return field.values?.find(([key]) => key === value)?.[1] ?? String(value);
    }
    if (value === true) {
        return "Yes";
    }
    return String(value);
}

/** A round number at least `max`, for a chart's scale to end on: 60 000 for 51 226. */
export function scaleTop(max: number): number {
    if (max <= 0) {
        return 1;
    }
    const power = 10 ** Math.floor(Math.log10(max));
    const step = [1, 2, 2.5, 5, 10].find((candidate) => candidate * power >= max) ?? 10;
    return step * power;
}
