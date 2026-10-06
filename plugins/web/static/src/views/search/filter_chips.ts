import { Component, computed, inject, load, loading, props, resource, t } from "trame";
import { and } from "@web/core/domain";
import { type Domain, Orm } from "@web/core/orm";
import { type Facet, type SearchView, withFilterToggled } from "./search_model";

/**
 * The filters of a search view as chips under the search, each saying how many records it finds
 * among the action's — counted together, in one call — and ticked or unticked in one click, as
 * in the filters menu. `All` unticks them all.
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
    });

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
            ],
        }),
        ({ model, domains }) => (domains.length < 2 ? Promise.resolve([]) : this.orm.countEach(model, domains)),
    );

    countAt(at: number): string {
        if (loading(() => this.counts)) {
            return "";
        }
        const count = this.counts?.[at];
        return count === undefined ? "" : String(count);
    }

    toggle(name: string): void {
        this.props.onChange(withFilterToggled(this.facets, name));
    }

    showAll(): void {
        this.props.onChange(this.facets.filter((facet) => facet.kind !== "filters"));
    }
}
