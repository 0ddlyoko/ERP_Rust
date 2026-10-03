import { Component, props, state, t } from "trame";
import {
    accepts,
    type Facet,
    facetLabel,
    type SearchField,
    type SearchView,
    withFilterToggled,
    withText,
} from "./search_model";

/**
 * A view's search: facets as chips, an input whose text is searched in the field the user picks
 * — the first one on Enter — and the view's filters to tick. Backspace in the empty input takes
 * the last facet off.
 */
export class SearchBar extends Component {
    static template = "web.SearchBar";

    props = props({
        view: t.any<SearchView>(),
        facets: t.array(t.any<Facet>()),
        onChange: t.func<(facets: Facet[]) => void>(),
    });

    @state accessor text = "";
    @state accessor active = 0;
    @state accessor filtersOpen = false;

    /** The fields the text can be searched in. */
    get suggestions(): SearchField[] {
        const text = this.text.trim();
        return text === "" ? [] : this.props.view.fields.filter((field) => accepts(field, text));
    }

    labelOf(facet: Facet): string {
        return facetLabel(this.props.view, facet);
    }

    isTicked(name: string): boolean {
        return this.props.facets.some((facet) => facet.kind === "filters" && facet.names.includes(name));
    }

    type(text: string): void {
        this.text = text;
        this.active = 0;
    }

    pick(field: SearchField): void {
        this.props.onChange(withText([...this.props.facets], field.name, this.text.trim()));
        this.text = "";
    }

    remove(at: number): void {
        this.props.onChange(this.props.facets.filter((_, index) => index !== at));
    }

    toggle(name: string): void {
        this.props.onChange(withFilterToggled([...this.props.facets], name));
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
