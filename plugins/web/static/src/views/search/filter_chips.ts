import { Component, computed, inject, load, loading, props, resource, t, untrack } from "trame";
import { and } from "@web/core/domain";
import { type Domain, Orm } from "@web/core/orm";
import { type Facet, type Favorite, type SearchView, searchDomain, withFilterToggled } from "./search_model";

/**
 * The filters of a search view as chips under the search, each saying how many records it finds
 * among the action's — counted together, in one call — and ticked or unticked in one click, as
 * in the filters menu. `All` unticks them all. The searches the user saved follow, each applied
 * in one click. Records the view created since are added to `All` as they are, rather than
 * counted again.
 */
export class FilterChips extends Component {
    static template = "web.FilterChips";

    props = props({
        model: t.string(),
        /** The action's records, which the counts are taken among. */
        domain: t.array(t.any()).default([]),
        view: t.any<SearchView>(),
        facets: t.array(t.any<Facet>()),
        onChange: t.func<(facets: Facet[]) => void>(),
        /** How many records the view created so far, of which those since the counts are added. */
        added: t.number().default(0),
        /** The searches the user saved, shown after the filters, each with what it finds. */
        favorites: t.array(t.any<Favorite>()).default([]),
        onApplyFavorite: t.func<(favorite: Favorite) => void>().optional(),
    });

    /** How many the view had created when the counts were taken. */
    private addedBefore = 0;

    @inject(Orm) orm!: Orm;

    get filters(): SearchView["filters"] {
        return (this.props.view as SearchView).filters;
    }

    /** The search's facets, as the search model takes them. */
    private get facets(): Facet[] {
        return this.props.facets as Facet[];
    }

    /** The filters ticked, by name. */
    @computed get ticked(): readonly string[] {
        const facet = this.facets.find((candidate) => candidate.kind === "filters");
        return facet?.kind === "filters" ? facet.names : [];
    }

    /** How many records the action has, then each filter finds among them. */
    @resource accessor counts: number[] = load(
        () => ({
            model: this.props.model,
            domains: [
                [...this.props.domain] as Domain,
                ...this.filters.map((filter) => and([[...this.props.domain], filter.domain])),
                ...this.savedSearches.map((favorite) =>
                    and([[...this.props.domain], searchDomain(this.props.view as SearchView, favorite.facets)]),
                ),
            ],
        }),
        ({ model, domains }) => {
            this.addedBefore = untrack(() => this.props.added);
            return domains.length < 2 ? Promise.resolve([]) : this.orm.countEach(model, domains);
        },
    );

    countAt(at: number): string {
        if (loading(() => this.counts)) {
            return "";
        }
        const count = this.counts?.[at];
        if (count === undefined) {
            return "";
        }
        return String(at === 0 ? count + this.props.added - this.addedBefore : count);
    }

    /** The saved searches shown as chips. */
    get savedSearches(): Favorite[] {
        return this.props.favorites as Favorite[];
    }

    /** Where a saved search's count is, among the counts. */
    savedAt(index: number): number {
        return 1 + this.filters.length + index;
    }

    /** Whether the search stands as a saved one has it. */
    isApplied(favorite: Favorite): boolean {
        return JSON.stringify(this.facets) === JSON.stringify(favorite.facets);
    }

    toggle(name: string): void {
        this.props.onChange(withFilterToggled(this.facets, name));
    }

    showAll(): void {
        this.props.onChange(this.facets.filter((facet) => facet.kind !== "filters"));
    }
}
