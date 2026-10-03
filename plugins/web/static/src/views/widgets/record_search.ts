import { Component, inject, props, state, t } from "trame";
import { Orm } from "@web/core/orm";

/** How long typing has to pause before the records are searched, in milliseconds. */
const SEARCH_AFTER = 250;

/** A record as a name search finds it: its id and its name. */
export type Choice = [number, string];

/**
 * An input finding records of a model by name, for the user to choose one.
 *
 * Entering it lists the first records at once; typing searches them by name, once typing pauses.
 * One is chosen with the mouse, or the arrows and Enter. Shows `text` while the user is not
 * typing; `onEmpty` is called as soon as they empty it.
 */
export class RecordSearch extends Component {
    static template = "web.RecordSearch";

    props = props({
        model: t.string(),
        onPick: t.func<(choice: Choice) => void>(),
        text: t.string().default(""),
        /** Records not offered: those already chosen. */
        exclude: t.array(t.number()).default([]),
        placeholder: t.string().default(""),
        onEmpty: t.func<() => void>().optional(),
    });

    @inject(Orm) orm!: Orm;

    /** What is typed, while the user types; `text` otherwise. */
    @state accessor query: string | null = null;
    @state accessor results: Choice[] = [];
    @state accessor active = 0;
    @state accessor isOpen = false;
    @state accessor searching = false;

    private timer: ReturnType<typeof setTimeout> | undefined;
    private searches = 0;

    get inputText(): string {
        return this.query ?? this.props.text;
    }

    /** What was found, less the records not offered. */
    get choices(): Choice[] {
        return this.results.filter(([id]) => !this.props.exclude.includes(id));
    }

    /** Entering the input: the first records listed at once, its text selected to type over. */
    enter(input: HTMLInputElement): void {
        input.select();
        if (this.isOpen) {
            return;
        }
        this.isOpen = true;
        void this.search("");
    }

    /** Typing searches once it pauses; emptying the input says so at once. */
    type(text: string): void {
        this.query = text;
        if (text === "") {
            this.props.onEmpty?.();
        }
        this.isOpen = true;
        this.searching = true;
        clearTimeout(this.timer);
        this.timer = setTimeout(() => void this.search(text), SEARCH_AFTER);
    }

    /**
     * Search what was typed; an answer to a search overtaken by another is dropped.
     *
     * Asks for as many more records as are not offered, so excluding some still fills the list.
     */
    async search(text: string): Promise<void> {
        const search = ++this.searches;
        this.searching = true;
        try {
            const found = await this.orm.nameSearch(this.props.model, text, 8 + this.props.exclude.length);
            if (search === this.searches) {
                this.results = found;
                this.active = 0;
            }
        } finally {
            if (search === this.searches) {
                this.searching = false;
            }
        }
    }

    pick(choice: Choice): void {
        this.props.onPick(choice);
        this.close();
    }

    close(): void {
        clearTimeout(this.timer);
        this.searches++;
        this.query = null;
        this.isOpen = false;
        this.results = [];
        this.searching = false;
    }

    key(event: KeyboardEvent): void {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            if (!this.isOpen) {
                this.type(this.inputText === this.props.text ? "" : this.inputText);
                return;
            }
            const step = event.key === "ArrowDown" ? 1 : -1;
            const count = this.choices.length;
            this.active = count === 0 ? 0 : (this.active + step + count) % count;
        } else if (event.key === "Enter" && this.isOpen) {
            event.preventDefault();
            const choice = this.choices[this.active];
            if (choice !== undefined) {
                this.pick(choice);
            }
        } else if (event.key === "Escape" && this.isOpen) {
            event.preventDefault();
            this.close();
        }
    }
}
