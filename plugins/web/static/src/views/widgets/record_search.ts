import { Component, effect, inject, props, state, t } from "trame";
import { Orm } from "@web/core/orm";

/** How long typing has to pause before the records are searched, in milliseconds. */
const SEARCH_AFTER = 250;

/** A record as a name search finds it: its id and its name. */
export type Choice = [number, string];

/** What the list offers: a record found, or creating one from what was typed. */
export type Entry = { kind: "pick"; choice: Choice } | { kind: "create" | "createEdit"; name: string };

/**
 * An input finding records of a model by name, for the user to choose one.
 *
 * Entering it lists the first records at once; typing searches them by name, once typing pauses.
 * One is chosen with the mouse, or the arrows and Enter. Shows `text` while the user is not
 * typing; `onEmpty` is called as soon as they empty it. With `onCreate` or `onCreateEdit`, what
 * is typed can also become a new record: created at once, or in a form first.
 */
export class RecordSearch extends Component {
    static template = "web.RecordSearch";

    props = props({
        model: t.string(),
        onPick: t.func<(choice: Choice) => void>(),
        text: t.string().default(""),
        /** Records not offered: those already chosen. */
        exclude: t.array(t.number()).default([]),
        /** Only the records matching it are offered. */
        domain: t.array(t.any()).default([]),
        placeholder: t.string().default(""),
        onEmpty: t.func<() => void>().optional(),
        onCreate: t.func<(name: string) => void>().optional(),
        onCreateEdit: t.func<(name: string) => void>().optional(),
    });

    @inject(Orm) orm!: Orm;

    /** What is typed, while the user types; `text` otherwise. */
    @state accessor query: string | null = null;
    @state accessor results: Choice[] = [];
    @state accessor active = 0;
    @state accessor isOpen = false;
    @state accessor searching = false;
    /** Where the list of records stands on the page: under the input, as wide as it. */
    @state accessor place = "";

    /** The input, set by the template. */
    input: HTMLInputElement | null = null;

    private timer: ReturnType<typeof setTimeout> | undefined;
    private searches = 0;

    get inputText(): string {
        return this.query ?? this.props.text;
    }

    /** What was found, less the records not offered. */
    get choices(): Choice[] {
        return this.results.filter(([id]) => !this.props.exclude.includes(id));
    }

    /**
     * While open, the list follows its input when the page or a box around it scrolls. It is
     * shown over the page rather than inside the input's box, which could clip it.
     */
    @effect followInput(): (() => void) | void {
        if (!this.isOpen) {
            return;
        }
        const follow = (): void => {
            const box = this.input?.getBoundingClientRect();
            if (box !== undefined) {
                this.place = `top: ${box.bottom + 4}px; left: ${box.left}px; width: ${box.width}px`;
            }
        };
        follow();
        window.addEventListener("scroll", follow, true);
        window.addEventListener("resize", follow);
        return () => {
            window.removeEventListener("scroll", follow, true);
            window.removeEventListener("resize", follow);
        };
    }

    /** The records found, then creating one from what was typed, when that is offered. */
    get entries(): Entry[] {
        const entries: Entry[] = this.choices.map((choice) => ({ kind: "pick", choice }));
        const name = (this.query ?? "").trim();
        if (name !== "" && this.props.onCreate !== undefined) {
            entries.push({ kind: "create", name });
        }
        if (name !== "" && this.props.onCreateEdit !== undefined) {
            entries.push({ kind: "createEdit", name });
        }
        return entries;
    }

    label(entry: Entry): string {
        switch (entry.kind) {
            case "pick":
                return entry.choice[1];
            case "create":
                return `Create "${entry.name}"`;
            case "createEdit":
                return "Create and edit…";
        }
    }

    choose(entry: Entry): void {
        switch (entry.kind) {
            case "pick":
                this.pick(entry.choice);
                return;
            case "create":
                this.props.onCreate?.(entry.name);
                break;
            case "createEdit":
                this.props.onCreateEdit?.(entry.name);
                break;
        }
        this.close();
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
            const found = await this.orm.nameSearch(
                this.props.model,
                text,
                8 + this.props.exclude.length,
                [...this.props.domain],
            );
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
            const count = this.entries.length;
            this.active = count === 0 ? 0 : (this.active + step + count) % count;
        } else if (event.key === "Enter" && this.isOpen) {
            event.preventDefault();
            const entry = this.entries[this.active];
            if (entry !== undefined) {
                this.choose(entry);
            }
        } else if (event.key === "Escape" && this.isOpen) {
            event.preventDefault();
            this.close();
        }
    }
}
