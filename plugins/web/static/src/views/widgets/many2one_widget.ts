import { inject, props, state } from "trame";
import { actionsOf, Menus } from "@web/core/menus";
import { Orm } from "@web/core/orm";
import { Router } from "@web/core/router";
import { Widget, widgetProps, widgets } from "./widget";

/** How long typing has to pause before the records are searched, in milliseconds. */
const SEARCH_AFTER = 250;

/** A record as a name search finds it: its id and its name. */
type Choice = [number, string];

/**
 * The record a many2one points to: its name when the view read it with names, as `[id, name]`;
 * its id otherwise, or when the user may not read it.
 *
 * Where a view edits it, entering it lists the first records of its model at once; typing
 * searches them by name, once typing pauses. One is chosen with the mouse, or the arrows and
 * Enter. Emptying it clears it.
 * Its record opens in a form when a menu leads to its model.
 */
export class Many2OneWidget extends Widget {
    static override template = "web.Many2OneWidget";

    override props = props({ ...widgetProps });

    @inject(Orm) orm!: Orm;
    @inject(Router) router!: Router;
    @inject(Menus) menus!: Menus;

    /** What is typed, while the user types; the record's name otherwise. */
    @state accessor query: string | null = null;
    @state accessor results: Choice[] = [];
    @state accessor active = 0;
    @state accessor isOpen = false;
    @state accessor searching = false;

    private timer: ReturnType<typeof setTimeout> | undefined;
    private searches = 0;

    override get text(): string {
        if (this.isEmpty) {
            return "";
        }
        if (Array.isArray(this.value)) {
            const [id, name] = this.value as [number, string | null];
            return name ?? `#${id}`;
        }
        return `#${this.value}`;
    }

    get id(): number | null {
        if (this.isEmpty) {
            return null;
        }
        return Array.isArray(this.value) ? (this.value[0] as number) : (this.value as number);
    }

    get inputText(): string {
        return this.query ?? this.text;
    }

    /** The action a menu opens on the model pointed to, to open its record with. */
    get openAction(): string | null {
        const model = this.props.field.relation;
        const action = actionsOf(this.menus.tree ?? []).find((action) => action.model === model);
        return action === undefined ? null : (action.xml_id ?? String(action.id));
    }

    /** Entering the input: the first records listed at once, its name selected to type over. */
    enter(input: HTMLInputElement): void {
        input.select();
        if (this.isOpen) {
            return;
        }
        this.isOpen = true;
        this.searching = true;
        void this.search("");
    }

    /** Typing searches once it pauses; emptying the input clears the field at once. */
    type(text: string): void {
        this.query = text;
        if (text === "" && !this.isEmpty) {
            this.props.onChange?.(null);
        }
        this.isOpen = true;
        this.searching = true;
        clearTimeout(this.timer);
        this.timer = setTimeout(() => void this.search(text), SEARCH_AFTER);
    }

    /** Search what was typed; an answer to a search overtaken by another is dropped. */
    async search(text: string): Promise<void> {
        const model = this.props.field.relation;
        if (model === undefined) {
            return;
        }
        const search = ++this.searches;
        this.searching = true;
        try {
            const found = await this.orm.nameSearch(model, text);
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
        this.props.onChange?.(choice);
        this.close();
    }

    /** Leave the input: what was typed without being chosen gives way to the record's name. */
    leave(): void {
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
                this.type(this.inputText === this.text ? "" : this.inputText);
                return;
            }
            const step = event.key === "ArrowDown" ? 1 : -1;
            const count = this.results.length;
            this.active = count === 0 ? 0 : (this.active + step + count) % count;
        } else if (event.key === "Enter" && this.isOpen) {
            event.preventDefault();
            const choice = this.results[this.active];
            if (choice !== undefined) {
                this.pick(choice);
            }
        } else if (event.key === "Escape" && this.isOpen) {
            event.preventDefault();
            this.close();
        }
    }

    openRecord(): void {
        const action = this.openAction;
        const id = this.id;
        if (action !== null && id !== null) {
            void this.router.go({ action, view: "form", id });
        }
    }
}

widgets.add("many2one", Many2OneWidget);
