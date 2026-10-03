import type { Domain } from "./orm";

/**
 * A domain as one expression: `[a, b]` — an implicit AND of two conditions — becomes
 * `["&", a, b]`, so it can stand as one operand of `&` or `|`.
 */
function asOne(domain: Domain): Domain {
    let expressions = 0;
    for (const term of [...domain].reverse()) {
        if (term === "&" || term === "|") {
            expressions -= 1;
        } else if (term !== "!") {
            expressions += 1;
        }
    }
    return [...Array<string>(Math.max(0, expressions - 1)).fill("&"), ...domain];
}

/** Records matching every domain; an empty list matches everything. */
export function and(domains: Domain[]): Domain {
    return domains.filter((domain) => domain.length > 0).flatMap(asOne);
}

/** Records matching any of the domains; none matches nothing. */
export function or(domains: Domain[]): Domain {
    if (domains.some((domain) => domain.length === 0)) {
        return [];
    }
    if (domains.length === 0) {
        return [["id", "in", []]];
    }
    return [...Array<string>(domains.length - 1).fill("|"), ...domains.flatMap(asOne)];
}

/** A `like` pattern for text appearing anywhere: `%` and `_` typed are matched as themselves. */
export function containing(text: string): string {
    return `%${text.replace(/[\\%_]/g, (character) => `\\${character}`)}%`;
}
