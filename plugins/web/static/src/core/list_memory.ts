import type { Domain } from "@web/core/orm";
import type { Facet } from "@web/views/search/search_model";

/** How a list is sorted: by one field, smallest or largest first. */
export interface Sort {
    name: string;
    descending: boolean;
}

/**
 * A list as the user left it: which page, which rows selected, the records it showed, how it was
 * searched, sorted and gathered — what the records beside a form follow.
 */
export interface ListMemory {
    offset: number;
    selected: number[];
    facets: Facet[];
    sort: Sort | null;
    /** The records of the page shown, in order: what a form steps through. */
    ids: number[];
    total: number;
    /** The records it found, as the search narrowed them. */
    domain: Domain;
    /** How it sorted them: `["date_order desc"]`, or the model's own order. */
    order: string[] | undefined;
    /** What it gathered them by: `state`, `date_order:month`, or nothing. */
    groupBy: string | null;
}

const memories = new Map<string, ListMemory>();

/**
 * A list as the user left it, by its [`listKey`](./router.ts): an action's, or some of its
 * records', so going back to it finds it as it was.
 */
export function listMemory(key: string | null): ListMemory | undefined {
    return key === null ? undefined : memories.get(key);
}

export function rememberList(key: string | null, memory: ListMemory): void {
    if (key !== null) {
        memories.set(key, memory);
    }
}
