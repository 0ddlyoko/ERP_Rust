import { registry } from "trame";
import type { FieldDescription } from "../../core/orm";

/** How a value of one type of field reads as text. */
export type Formatter = (value: unknown, field: FieldDescription) => string;

/** Formatters by field type. A plugin adds its own type, or replaces how one reads, here. */
export const formatters = registry.category<Formatter>("formatters");

const empty = (value: unknown) => value === null || value === undefined || value === "";

/** A date as the server writes it, `YYYY-MM-DD`, read in the browser's time zone rather than UTC. */
function localDate(value: string): Date {
    const [year, month, day] = value.split("-").map(Number);
    return new Date(year, month - 1, day);
}

formatters
    .add("string", (value) => (empty(value) ? "" : String(value)))
    .add("integer", (value) => (empty(value) ? "" : String(value)))
    .add("decimal", (value) => (empty(value) ? "" : Number(value).toLocaleString()))
    .add("bool", (value) => (value ? "✓" : ""))
    .add("date", (value) => (empty(value) ? "" : localDate(String(value)).toLocaleDateString()))
    .add("datetime", (value) => (empty(value) ? "" : new Date(String(value)).toLocaleString()))
    .add("ref", (value) => (empty(value) ? "" : `#${value}`))
    .add("refs", (value) => (Array.isArray(value) ? `${value.length} record(s)` : ""));

/** A value as text, the way its field reads: by its type's formatter, or as it is. */
export function formatValue(value: unknown, field: FieldDescription): string {
    const formatter = formatters.get(field.type, null);
    if (formatter !== null) {
        return formatter(value, field);
    }
    return empty(value) ? "" : String(value);
}
