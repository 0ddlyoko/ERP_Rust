import { Component, type ComponentClass, computed, effect, inject, load, loading, props, resource, state, t } from "trame";
import { Icon } from "@web/core/icons";
import { type ActionDescription, actionsOf, holds, leadsTo, type MenuEntry, pathsOf } from "@web/core/menus";
import { Orm } from "@web/core/orm";
import { Session } from "@web/core/session";
import { systray } from "./systray";

/** Where the browser remembers the menu folded. */
const FOLDED_KEY = "o_sidebar_folded";

/** How long the pointer rests on the narrow menu, or leaves the opened one, before it changes. */
const HOVER_DELAY = 150;

function readFolded(): boolean {
    try {
        return localStorage.getItem(FOLDED_KEY) === "1";
    } catch {
        return false;
    }
}

/**
 * The menu on the left: the module chosen, its menus with how many records each shows, a search
 * over every module's, the other modules, what plugins put in the tray, and who is logged in.
 *
 * Under a module come groups the user opens and folds — the one leading to the action open starts
 * unfolded. In a group, an entry with entries of its own is a section heading over them.
 *
 * Narrow — icons only — while a record is open, or always once the user folded it, which the
 * browser remembers. Narrow, it opens over the page while the pointer rests on it or the keyboard
 * is in it, a moment after either comes, and closes a moment after both left.
 */
export class Sidebar extends Component {
    static template = "web.Sidebar";
    static components = { Icon };

    props = props({
        modules: t.array(t.any<MenuEntry>()),
        module: t.any<MenuEntry | null>(),
        action: t.any<ActionDescription | null>(),
        /** The entry the action was opened from, when it was. */
        menu: t.number().orNull().default(null),
        /** Whether a record is open, which wants the page's width. */
        compact: t.boolean().default(false),
        onModule: t.func<(module: MenuEntry) => void>(),
        onOpen: t.func<(entry: MenuEntry) => void>(),
        /** Called when the user asks for the home page. */
        onHome: t.func<() => void>().optional(),
    });

    @inject(Orm) orm!: Orm;
    @inject(Session) session!: Session;

    @state accessor switching = false;
    @state accessor query = "";
    @state accessor folding = new Map<number, boolean>();
    @state accessor folded = readFolded();
    @state accessor hovered = false;
    @state accessor focused = false;

    private hoverTimer: ReturnType<typeof setTimeout> | undefined;

    get modules(): readonly MenuEntry[] {
        return this.props.modules;
    }

    get module(): MenuEntry | null {
        return this.props.module;
    }

    /** The modules but the one shown, to switch to in one click. */
    get others(): readonly MenuEntry[] {
        return this.props.modules.filter((module) => module.id !== this.props.module?.id);
    }

    /** The groups of the module shown. */
    get groups(): readonly MenuEntry[] {
        return this.props.module?.children ?? [];
    }

    entriesOf(entry: MenuEntry): readonly MenuEntry[] {
        return entry.children;
    }

    /** What plugins put in the tray, by their key. */
    get tray(): [string, ComponentClass][] {
        return systray.getEntries();
    }

    /** Whether the menu keeps to its icons: a record is open or the user folded it. */
    get narrow(): boolean {
        return this.props.compact || this.folded;
    }

    /** Whether it shows its names: wide, or narrow and opened over the page. */
    get wide(): boolean {
        return !this.narrow || this.hovered || this.focused;
    }

    /** Fold the menu into a strip, or unfold it, and remember it so. */
    fold(): void {
        this.folded = !this.folded;
        this.hovered = false;
        try {
            localStorage.setItem(FOLDED_KEY, this.folded ? "1" : "0");
        } catch {
            // Kept for this page only when the browser keeps nothing.
        }
    }

    /** Open the narrow menu, or keep it closed, once the pointer stayed where it is. */
    hover(over: boolean): void {
        clearTimeout(this.hoverTimer);
        this.hoverTimer = setTimeout(() => {
            this.hovered = over;
        }, HOVER_DELAY);
    }

    /**
     * The keyboard came into the menu, or left it for good. A click focuses what it clicks too,
     * which must not hold the menu open once the pointer left: only a focus the browser would
     * show — the keyboard's — counts.
     */
    focus(within: boolean, event: FocusEvent): void {
        const nav = event.currentTarget as HTMLElement;
        if (!within && nav.contains(event.relatedTarget as Node | null)) {
            return;
        }
        this.focused = within && (event.target as HTMLElement).matches(":focus-visible");
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

    /** Whether a group holds the entry open, to mark its icon when the menu is narrow. */
    holdsActive(group: MenuEntry): boolean {
        return this.isActive(group) || group.children.some((child) => this.holdsActive(child));
    }

    href(entry: MenuEntry): string {
        return entry.action ? `#action=${entry.action.xml_id ?? entry.action.id}` : "#";
    }

    /** The list of modules closes on a press anywhere outside the switcher, or on Escape. */
    @effect closeSwitcherOutside(): (() => void) | void {
        if (!this.switching) {
            return;
        }
        const press = (event: MouseEvent): void => {
            if (!(event.target instanceof Element && event.target.closest(".o_module_switcher"))) {
                this.switching = false;
            }
        };
        const key = (event: KeyboardEvent): void => {
            if (event.key === "Escape") {
                this.switching = false;
            }
        };
        document.addEventListener("mousedown", press);
        document.addEventListener("keydown", key);
        return () => {
            document.removeEventListener("mousedown", press);
            document.removeEventListener("keydown", key);
        };
    }

    pick(module: MenuEntry): void {
        this.switching = false;
        this.query = "";
        this.props.onModule(module);
    }

    home(): void {
        this.switching = false;
        this.props.onHome?.();
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

    /** The actions of the module shown, each once. */
    @computed get actionIds(): number[] {
        const ids = actionsOf(this.groups).map((action) => action.id);
        return [...new Set(ids)].sort((left, right) => left - right);
    }

    /** How many records each action of the module shows, by its id: one call per module. */
    @resource accessor counts: Record<string, number | null> = load(
        () => this.actionIds,
        (ids) => (ids.length === 0 ? Promise.resolve({}) : this.orm.call<Record<string, number | null>>("action", "counts", ids)),
    );

    /** An entry's count, nothing while it comes or when it cannot be had. */
    countOf(entry: MenuEntry): string {
        if (entry.action === null || loading(() => this.counts)) {
            return "";
        }
        const count = this.counts?.[String(entry.action.id)];
        return count === null || count === undefined ? "" : String(count);
    }

    /** A module's colour, as its tile is painted. */
    tileStyle(module: MenuEntry | null): string {
        return module?.color ? `background: ${module.color}` : "";
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
