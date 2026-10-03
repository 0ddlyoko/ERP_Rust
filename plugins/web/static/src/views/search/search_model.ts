import { and, containing, or } from "@web/core/domain";
import type { Fields } from "@web/core/models";
import type { Domain, FieldDescription } from "@web/core/orm";

/** A field typing searches in, as a `<field>` of the search view says. */
export interface SearchField {
    name: string;
    label: string;
    field: FieldDescription;
    /** What the records found must also match, as the view's `domain` says. */
    domain: Domain;
}

/** A filter to tick, as a `<filter>` of the search view says. */
export interface SearchFilter {
    name: string;
    label: string;
    domain: Domain;
    isDefault: boolean;
}

/** What a search view offers. */
export interface SearchView {
    fields: SearchField[];
    filters: SearchFilter[];
}

/**
 * One condition of a search, shown as a chip: texts searched in one field, any of them; or
 * filters ticked, any of them.
 */
export type Facet = { kind: "field"; name: string; values: string[] } | { kind: "filters"; names: string[] };

/** Field types a text is searched in. */
const SEARCHABLE = new Set(["string", "ref", "refs", "selection", "integer", "decimal"]);

/** The search view's XML, read against the model's fields: those it cannot search in are left out. */
export function readSearchView(arch: string, fields: Fields): SearchView {
    const root = new DOMParser().parseFromString(arch, "text/xml").documentElement;
    const domainOf = (element: Element): Domain => JSON.parse(element.getAttribute("domain") ?? "[]") as Domain;
    const searchFields: SearchField[] = [];
    const filters: SearchFilter[] = [];
    for (const element of Array.from(root.children)) {
        const name = element.getAttribute("name") ?? "";
        if (element.tagName === "field") {
            const field = fields[name];
            if (field !== undefined && SEARCHABLE.has(field.type)) {
                searchFields.push({
                    name,
                    label: element.getAttribute("string") ?? field.label,
                    field,
                    domain: domainOf(element),
                });
            }
        } else if (element.tagName === "filter") {
            filters.push({
                name,
                label: element.getAttribute("string") ?? name,
                domain: domainOf(element),
                isDefault: element.getAttribute("default") === "1",
            });
        }
    }
    return { fields: searchFields, filters };
}

/** The filters a search starts with: those ticked by default. */
export function defaultFacets(view: SearchView): Facet[] {
    const names = view.filters.filter((filter) => filter.isDefault).map((filter) => filter.name);
    return names.length === 0 ? [] : [{ kind: "filters", names }];
}

/**
 * Whether a text can be searched in a field: a number field needs a number; a selection, a
 * label holding the text.
 */
export function accepts(searchField: SearchField, text: string): boolean {
    const { field } = searchField;
    if (field.type === "integer" || field.type === "decimal") {
        return text.trim() !== "" && Number.isFinite(Number(text));
    }
    return true;
}

/** The records a text found in a field: holding it, by name for a relation, by label for a selection. */
function textDomain(searchField: SearchField, text: string): Domain {
    const { name, field } = searchField;
    let condition: Domain;
    if (field.type === "selection") {
        const lowered = text.toLowerCase();
        const keys = (field.values ?? []).filter(([, label]) => label.toLowerCase().includes(lowered)).map(([key]) => key);
        condition = [[name, "in", keys]];
    } else if (field.type === "integer" || field.type === "decimal") {
        condition = [[name, "=", Number(text)]];
    } else {
        condition = [[name, "ilike", containing(text)]];
    }
    return and([condition, searchField.domain]);
}

/** The records a search finds: every facet, each matching any of its values. */
export function searchDomain(view: SearchView, facets: Facet[]): Domain {
    return and(
        facets.map((facet) => {
            if (facet.kind === "filters") {
                return or(view.filters.filter((filter) => facet.names.includes(filter.name)).map((filter) => filter.domain));
            }
            const searchField = view.fields.find((candidate) => candidate.name === facet.name);
            return searchField === undefined ? [] : or(facet.values.map((value) => textDomain(searchField, value)));
        }),
    );
}

/** A facet as its chip reads: `Login: ad or ma`, `Active or Archived`. */
export function facetLabel(view: SearchView, facet: Facet): string {
    if (facet.kind === "filters") {
        return facet.names.map((name) => view.filters.find((filter) => filter.name === name)?.label ?? name).join(" or ");
    }
    const label = view.fields.find((field) => field.name === facet.name)?.label ?? facet.name;
    return `${label}: ${facet.values.join(" or ")}`;
}

/** The facets once a text is searched in a field: added to that field's facet, or a new one. */
export function withText(facets: Facet[], name: string, text: string): Facet[] {
    const at = facets.findIndex((facet) => facet.kind === "field" && facet.name === name);
    if (at < 0) {
        return [...facets, { kind: "field", name, values: [text] }];
    }
    return facets.map((facet, index) =>
        index === at && facet.kind === "field" && !facet.values.includes(text)
            ? { ...facet, values: [...facet.values, text] }
            : facet,
    );
}

/** The facets once a filter is ticked or unticked; the filters facet goes when none is left. */
export function withFilterToggled(facets: Facet[], name: string): Facet[] {
    const current = facets.find((facet) => facet.kind === "filters");
    const names = current?.kind === "filters" ? current.names : [];
    const next = names.includes(name) ? names.filter((known) => known !== name) : [...names, name];
    if (next.length === 0) {
        return facets.filter((facet) => facet.kind !== "filters");
    }
    if (current === undefined) {
        return [...facets, { kind: "filters", names: next }];
    }
    return facets.map((facet) => (facet.kind === "filters" ? { kind: "filters", names: next } : facet));
}
