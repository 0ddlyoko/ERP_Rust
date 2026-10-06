/** Names an expression may read without being fields: the language's own, and a few globals. */
const EXPRESSION_WORDS = new Set([
    "true", "false", "null", "undefined", "typeof", "instanceof", "in", "of", "new", "void",
    "NaN", "Infinity", "Math", "Number", "String", "Boolean", "Array", "Date", "JSON", "Object",
]);

/**
 * The names a condition reads: what is left once strings, numbers, properties (`.includes`), the
 * language's words and the parameters of arrow functions (`x => x.id`) are set aside. The server
 * reads them the same way, to refuse one its model lacks.
 */
export function namesRead(expression: string): string[] {
    const parameters = new Set<string>();
    for (const match of expression.matchAll(/(?:\(([^()]*)\)|([A-Za-z_$][\w$]*))\s*=>/g)) {
        for (const name of (match[1] ?? match[2]).split(",")) {
            if (name.trim()) {
                parameters.add(name.trim());
            }
        }
    }
    const withoutStrings = expression.replace(/'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`/g, " ");
    const names: string[] = [];
    for (const match of withoutStrings.matchAll(/(\.\s*)?([A-Za-z_$][\w$]*)/g)) {
        const [, property, name] = match;
        const before = withoutStrings[match.index - 1];
        if (property || (before !== undefined && /\d/.test(before)) || EXPRESSION_WORDS.has(name) || parameters.has(name)) {
            continue;
        }
        if (!names.includes(name)) {
            names.push(name);
        }
    }
    return names;
}

/**
 * The value of an expression a view wrote — a condition, a decoration — with each field it
 * reads set to the record's value: a record by its id, records by their ids, as the form's
 * conditions read them. An expression that fails is false.
 */
export function evaluate(expression: string, values: Record<string, unknown>): unknown {
    const names = namesRead(expression);
    const read = names.map((name) => idsOf(values[name]));
    try {
        return new Function(...names, `return (${expression});`)(...read);
    } catch {
        return false;
    }
}

/** A record `[id, name]` as its id, records as their ids; anything else as it is. */
function idsOf(value: unknown): unknown {
    const isNamed = (item: unknown): boolean => Array.isArray(item) && item.length === 2 && typeof item[0] === "number";
    if (isNamed(value)) {
        return (value as [number, unknown])[0];
    }
    if (Array.isArray(value) && value.every(isNamed)) {
        return value.map((item) => (item as [number, unknown])[0]);
    }
    return value ?? null;
}

/** The colours a decoration attribute names: `decoration-success="state === 'done'"`. */
export const DECORATIONS = ["success", "info", "warning", "danger", "muted"] as const;

/** The first decoration whose expression holds for the record, among an element's attributes. */
export function decorationOf(attrs: Record<string, string>, values: Record<string, unknown>): string | null {
    for (const kind of DECORATIONS) {
        const expression = attrs[`decoration-${kind}`];
        if (expression !== undefined && evaluate(expression, values)) {
            return kind;
        }
    }
    return null;
}

/** The fields the decorations of these attributes read. */
export function decorationNames(attrs: Record<string, string>): string[] {
    return DECORATIONS.flatMap((kind) => {
        const expression = attrs[`decoration-${kind}`];
        return expression === undefined ? [] : namesRead(expression);
    });
}
