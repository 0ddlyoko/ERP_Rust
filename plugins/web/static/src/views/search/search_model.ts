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

/** A way to gather the records, as a `<filter group_by="…">` says, or a field offered for it. */
export interface SearchGroupBy {
    name: string;
    label: string;
    /** `state`, or `date_order:month` for a date by period. */
    groupBy: string;
}

/** A search the user saved on a list, as `saved_filter.mine` answers. */
export interface Favorite {
    id: number;
    name: string;
    facets: Facet[];
    /** The list opens with it. */
    is_default: boolean;
}

/** What a search view offers. */
export interface SearchView {
    fields: SearchField[];
    filters: SearchFilter[];
    /** The groupings the view names. */
    groupBys: SearchGroupBy[];
    /** Every field the records could be gathered by, for a grouping of the user's own. */
    groupable: SearchGroupBy[];
}

/**
 * One condition of a search, shown as a chip: texts searched in one field, any of them; or
 * filters ticked, any of them.
 */
export type Facet =
    | { kind: "field"; name: string; values: string[] }
    | { kind: "filters"; names: string[] }
    | { kind: "groupby"; groupBy: string; label: string };

/** Field types records can be gathered by: those holding one value each. */
const GROUPABLE = new Set(["string", "ref", "selection", "integer", "bool", "date", "datetime"]);

/** Field types a text is searched in. */
const SEARCHABLE = new Set(["string", "ref", "refs", "selection", "integer", "decimal"]);

/** The search view's XML, read against the model's fields: those it cannot search in are left out. */
export function readSearchView(arch: string, fields: Fields): SearchView {
    const root = new DOMParser().parseFromString(arch, "text/xml").documentElement;
    const domainOf = (element: Element): Domain => JSON.parse(element.getAttribute("domain") ?? "[]") as Domain;
    const searchFields: SearchField[] = [];
    const filters: SearchFilter[] = [];
    const groupBys: SearchGroupBy[] = [];
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
        } else if (element.tagName === "filter" && element.hasAttribute("group_by")) {
            const groupBy = element.getAttribute("group_by") ?? "";
            const field = fields[groupBy.split(":")[0]];
            groupBys.push({ name, label: element.getAttribute("string") ?? field?.label ?? groupBy, groupBy });
        } else if (element.tagName === "filter") {
            filters.push({
                name,
                label: element.getAttribute("string") ?? name,
                domain: domainOf(element),
                isDefault: element.getAttribute("default") === "1",
            });
        }
    }
    const groupable = Object.entries(fields)
        .filter(([name, field]) => field.stored && GROUPABLE.has(field.type) && name !== "id")
        .map(([name, field]) => ({
            name,
            label: field.label,
            groupBy: field.type === "date" || field.type === "datetime" ? `${name}:month` : name,
        }))
        .sort((left, right) => left.label.localeCompare(right.label));
    return { fields: searchFields, filters, groupBys, groupable };
}

/** How the records are gathered, if they are. */
export function groupByOf(facets: Facet[]): { groupBy: string; label: string } | null {
    const facet = facets.find((candidate) => candidate.kind === "groupby");
    return facet?.kind === "groupby" ? { groupBy: facet.groupBy, label: facet.label } : null;
}

/** The facets once the records are gathered by `groupBy`, or no longer, asked a second time. */
export function withGroupBy(facets: Facet[], groupBy: string, label: string): Facet[] {
    const others = facets.filter((facet) => facet.kind !== "groupby");
    return groupByOf(facets)?.groupBy === groupBy ? others : [...others, { kind: "groupby", groupBy, label }];
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
            if (facet.kind === "groupby") {
                return [];
            }
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
    if (facet.kind === "groupby") {
        return `Grouped by ${facet.label}`;
    }
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
