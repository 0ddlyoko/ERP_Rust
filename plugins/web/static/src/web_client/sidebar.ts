import { Component, computed, inject, props, state, t } from "trame";
import { type ActionDescription, holds, leadsTo, type MenuEntry, pathsOf } from "@web/core/menus";
import { Session } from "@web/core/session";

/**
 * The menu on the left: the module chosen, its menus, a search over every module's, and who is
 * logged in.
 *
 * Under a module come groups the user opens and folds — the one leading to the action open starts
 * unfolded. In a group, an entry with entries of its own is a section heading over them.
 *
 * The whole menu folds into a narrow strip, to give the page its width; the browser remembers
 * it folded.
 */
/** Where the browser remembers the menu folded. */
const FOLDED_KEY = "o_sidebar_folded";

function readFolded(): boolean {
    try {
        return localStorage.getItem(FOLDED_KEY) === "1";
    } catch {
        return false;
    }
}

export class Sidebar extends Component {
    static template = "web.Sidebar";

    props = props({
        modules: t.array(t.any<MenuEntry>()),
        module: t.any<MenuEntry | null>(),
        action: t.any<ActionDescription | null>(),
        /** The entry the action was opened from, when it was. */
        menu: t.number().orNull().default(null),
        onModule: t.func<(module: MenuEntry) => void>(),
        onOpen: t.func<(entry: MenuEntry) => void>(),
        /** Called when the user asks for the home page. */
        onHome: t.func<() => void>().optional(),
    });

    @inject(Session) session!: Session;

    get modules(): readonly MenuEntry[] {
        return this.props.modules;
    }

    get module(): MenuEntry | null {
        return this.props.module;
    }

    /** The groups of the module shown. */
    get groups(): readonly MenuEntry[] {
        return this.props.module?.children ?? [];
    }

    entriesOf(entry: MenuEntry): readonly MenuEntry[] {
        return entry.children;
    }

    @state accessor switching = false;
    @state accessor query = "";
    @state accessor folding = new Map<number, boolean>();
    @state accessor folded = readFolded();

    /** Fold the menu into a strip, or unfold it, and remember it so. */
    fold(): void {
        this.folded = !this.folded;
        try {
            localStorage.setItem(FOLDED_KEY, this.folded ? "1" : "0");
        } catch {
            // Kept for this page only when the browser keeps nothing.
        }
    }

    isOpen(group: MenuEntry): boolean {
        const action = this.props.action;
        const menu = this.props.menu;
        const leading = menu !== null ? holds(group, menu) : action !== null && leadsTo(group, action.id);
        return this.folding.get(group.id) ?? leading;
    }

    toggle(group: MenuEntry): void {
        this.folding.set(group.id, !this.isOpen(group));
    }

    /** The entry the action was opened from; failing that, every entry opening it. */
    isActive(entry: MenuEntry): boolean {
        if (this.props.menu !== null) {
            return entry.id === this.props.menu;
        }
        return entry.action !== null && entry.action.id === this.props.action?.id;
    }

    href(entry: MenuEntry): string {
        return entry.action ? `#action=${entry.action.xml_id ?? entry.action.id}` : "#";
    }

    pick(module: MenuEntry): void {
        this.switching = false;
        this.query = "";
        this.props.onModule(module);
    }

    open(entry: MenuEntry): void {
        this.query = "";
        this.props.onOpen(entry);
    }

    /** The entries of every module whose name or path holds what was typed. */
    @computed get results(): { entry: MenuEntry; path: string }[] {
        const query = this.query.trim().toLowerCase();
        if (!query) {
            return [];
        }
        return pathsOf(this.props.modules).filter(({ path }) => path.toLowerCase().includes(query));
    }

    initial(entry: MenuEntry | null): string {
        return entry?.name.trim().charAt(0).toUpperCase() ?? "";
    }

    get initials(): string {
        return this.session.name
            .split(/\s+/)
            .filter(Boolean)
            .slice(0, 2)
            .map((part) => part.charAt(0).toUpperCase())
            .join("");
    }

    get role(): string {
        return this.session.hasGroup("base.group_admin") ? "Administrator" : "User";
    }
}
