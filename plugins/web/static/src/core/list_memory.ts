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
    /** The groups it had open, by their key. */
    openGroups: string[];
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

/** How some records were last searched and gathered, by their list or their board alike. */
export interface SearchMemory {
    facets: Facet[];
    /** The records the search found. */
    domain: Domain;
    /** What they were gathered by — a board always gathers them, by its `default_group_by` at least. */
    grouping: { groupBy: string; label: string } | null;
}

const searches = new Map<string, SearchMemory>();

/** How the records of a list key were last searched, in whichever of their views. */
export function searchMemory(key: string | null): SearchMemory | undefined {
    return key === null ? undefined : searches.get(key);
}

export function rememberSearch(key: string | null, memory: SearchMemory): void {
    if (key !== null) {
        searches.set(key, memory);
    }
}

/** The search another view of the records left, for a view to open with: gathered as they were. */
export function searchFacets(key: string | null): Facet[] | undefined {
    const memory = searchMemory(key);
    if (memory === undefined) {
        return undefined;
    }
    const others = memory.facets.filter((facet) => facet.kind !== "groupby");
    return memory.grouping === null ? others : [...others, { kind: "groupby", ...memory.grouping }];
}

const openings = new Map<string, Facet[]>();

/** Open a list searched this way, once — the records a dashboard's figure stands for — over how it was left. */
export function openWith(key: string | null, facets: Facet[]): void {
    if (key !== null) {
        openings.set(key, facets);
    }
}

/** The search a list is opened with, if one was asked: given once, then forgotten. */
export function takeOpening(key: string | null): Facet[] | undefined {
    if (key === null) {
        return undefined;
    }
    const facets = openings.get(key);
    openings.delete(key);
    return facets;
}
