import { Component, effect, props, state, t } from "trame";
import {
    accepts,
    type Facet,
    facetLabel,
    groupByOf,
    type Favorite,
    type SearchField,
    type SearchGroupBy,
    type SearchView,
    withFilterToggled,
    withGroupBy,
    withText,
} from "./search_model";

/**
 * A view's search: facets as chips, an input whose text is searched in the field the user picks
 * — the first one on Enter — the view's filters to tick, and the groupings to gather the records
 * by: the view's, or any field's. Backspace in the empty input takes
 * the last facet off.
 */
export class SearchBar extends Component {
    static template = "web.SearchBar";

    props = props({
        view: t.any<SearchView>(),
        facets: t.array(t.any<Facet>()),
        onChange: t.func<(facets: Facet[]) => void>(),
        /** The searches the user saved, offered in the menu where the view saves them. */
        favorites: t.array(t.any<Favorite>()).default([]),
        onSaveFavorite: t.func<(name: string, isDefault: boolean) => void>().optional(),
        onForgetFavorite: t.func<(favorite: Favorite) => void>().optional(),
        onApplyFavorite: t.func<(favorite: Favorite) => void>().optional(),
    });

    @state accessor favoriteName = "";
    @state accessor favoriteDefault = false;

    /** Save the search under the name typed. */
    saveFavorite(): void {
        const name = this.favoriteName.trim();
        if (name === "" || this.props.onSaveFavorite === undefined) {
            return;
        }
        this.props.onSaveFavorite(name, this.favoriteDefault);
        this.favoriteName = "";
        this.favoriteDefault = false;
        this.filtersOpen = false;
    }

    applyFavorite(favorite: Favorite): void {
        this.props.onApplyFavorite?.(favorite);
        this.filtersOpen = false;
    }

    @state accessor text = "";
    @state accessor active = 0;
    @state accessor filtersOpen = false;

    /** The bar's element, set by its template. */
    element: HTMLElement | null = null;

    /** While the filters are open, pressing anywhere outside them closes them. */
    @effect closeFiltersFromOutside(): (() => void) | void {
        if (!this.filtersOpen) {
            return;
        }
        const close = (event: MouseEvent): void => {
            const target = event.target instanceof Element ? event.target : null;
            if (target?.closest(".o_search_filters") === null || target === null) {
                this.filtersOpen = false;
            }
        };
        document.addEventListener("mousedown", close, true);
        return () => document.removeEventListener("mousedown", close, true);
    }

    /** The fields the text can be searched in. */
    get suggestions(): SearchField[] {
        const text = this.text.trim();
        const fields = (this.props.view as SearchView).fields;
        return text === "" ? [] : fields.filter((field) => accepts(field, text));
    }

    labelOf(facet: Facet): string {
        return facetLabel(this.props.view as SearchView, facet);
    }

    isTicked(name: string): boolean {
        return this.props.facets.some((facet) => facet.kind === "filters" && facet.names.includes(name));
    }

    type(text: string): void {
        this.text = text;
        this.active = 0;
    }

    pick(field: SearchField): void {
        this.props.onChange(withText([...this.props.facets] as Facet[], field.name, this.text.trim()));
        this.text = "";
    }

    remove(at: number): void {
        this.props.onChange((this.props.facets as Facet[]).filter((_, index) => index !== at));
    }

    toggle(name: string): void {
        this.props.onChange(withFilterToggled([...this.props.facets] as Facet[], name));
    }

    isGroupedBy(groupBy: string): boolean {
        return groupByOf([...this.props.facets] as Facet[])?.groupBy === groupBy;
    }

    /** Gather the records as the option says; the same option again ungroups them. */
    groupBy(option: SearchGroupBy): void {
        this.props.onChange(withGroupBy([...this.props.facets] as Facet[], option.groupBy, option.label));
    }

    /** Gather the records by a field the user picked. */
    groupByField(groupBy: string): void {
        const option = this.props.view.groupable.find((candidate) => candidate.groupBy === groupBy);
        if (option !== undefined) {
            this.groupBy(option);
            this.filtersOpen = false;
        }
    }

    key(event: KeyboardEvent): void {
        const suggestions = this.suggestions;
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            const step = event.key === "ArrowDown" ? 1 : -1;
            this.active = suggestions.length === 0 ? 0 : (this.active + step + suggestions.length) % suggestions.length;
        } else if (event.key === "Enter") {
            const field = suggestions[this.active];
            if (field !== undefined) {
                event.preventDefault();
                this.pick(field);
            }
        } else if (event.key === "Escape") {
            this.text = "";
        } else if (event.key === "Backspace" && this.text === "" && this.props.facets.length > 0) {
            this.remove(this.props.facets.length - 1);
        }
    }
}
