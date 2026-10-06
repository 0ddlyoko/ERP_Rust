import { Component, computed, inject, load, loading, props, resource, t } from "trame";
import { actionsOf, type MenuEntry } from "@web/core/menus";
import { Orm } from "@web/core/orm";
import { Session } from "@web/core/session";

/** How many entries of a module its card lists. */
const ENTRIES_SHOWN = 5;

/**
 * Where the back office opens: a greeting, then a card per module with its first entries, each
 * saying how many records it shows — counted for every entry in one call.
 */
export class Home extends Component {
    static template = "web.Home";

    props = props({
        modules: t.array(t.any<MenuEntry>()),
        onOpen: t.func<(entry: MenuEntry) => void>(),
    });

    @inject(Orm) orm!: Orm;
    @inject(Session) session!: Session;

    get greeting(): string {
        const hour = new Date().getHours();
        const moment = hour < 12 ? "Good morning" : hour < 18 ? "Good afternoon" : "Good evening";
        return `${moment}, ${this.session.name.split(/\s+/)[0]}`;
    }

    get today(): string {
        return new Date().toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
    }

    /** Modules that open anything. */
    get modules(): readonly MenuEntry[] {
        return this.props.modules.filter((module) => actionsOf([module]).length > 0);
    }

    /** The entries of a module opening an action, in the menus' order. */
    entriesOf(module: MenuEntry): MenuEntry[] {
        const entries: MenuEntry[] = [];
        const walk = (entry: MenuEntry): void => {
            if (entry.action !== null) {
                entries.push(entry);
            }
            entry.children.forEach(walk);
        };
        module.children.forEach(walk);
        return entries.slice(0, ENTRIES_SHOWN);
    }

    /** The actions of every card, each once. */
    @computed get actionIds(): number[] {
        const ids = this.modules.flatMap((module) => this.entriesOf(module).map((entry) => entry.action?.id ?? 0));
        return [...new Set(ids.filter((id) => id > 0))].sort((left, right) => left - right);
    }

    /** How many records each action shows, by its id; `null` for one the user may not count. */
    @resource accessor counts: Record<string, number | null> = load(
        () => this.actionIds,
        (ids) => (ids.length === 0 ? Promise.resolve({}) : this.orm.call<Record<string, number | null>>("action", "counts", ids)),
    );

    /** An entry's count, `…` while it comes, nothing when it cannot be had. */
    countOf(entry: MenuEntry): string {
        if (loading(() => this.counts)) {
            return "…";
        }
        const count = this.counts?.[String(entry.action?.id)];
        return count === null || count === undefined ? "" : String(count);
    }

    hasSome(entry: MenuEntry): boolean {
        return !loading(() => this.counts) && (this.counts?.[String(entry.action?.id)] ?? 0) > 0;
    }

    initial(module: MenuEntry): string {
        return module.name.trim().charAt(0).toUpperCase();
    }
}
