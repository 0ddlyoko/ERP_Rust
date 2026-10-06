import { Component, inject, load, props, resource, t } from "trame";
import { type ActionDescription, actionsOf, type MenuEntry } from "@web/core/menus";
import { Orm } from "@web/core/orm";
import { Session } from "@web/core/session";

/** How many entries of a module its card lists. */
const ENTRIES_SHOWN = 5;

/** How many records an action shows, as a number beside its entry. */
export class ActionCount extends Component {
    static template = "web.ActionCount";

    props = props({ action: t.any<ActionDescription>() });

    @inject(Orm) orm!: Orm;

    @resource accessor count: number = load(
        () => this.props.action,
        (action) => this.orm.count(action.model, [...action.domain]),
    );
}

/**
 * Where the back office opens: a greeting, then a card per module with its first entries, each
 * saying how many records it shows.
 */
export class Home extends Component {
    static template = "web.Home";
    static components = { ActionCount };

    props = props({
        modules: t.array(t.any<MenuEntry>()),
        onOpen: t.func<(entry: MenuEntry) => void>(),
    });

    @inject(Session) session!: Session;

    get greeting(): string {
        const hour = new Date().getHours();
        const moment = hour < 12 ? "Good morning" : hour < 18 ? "Good afternoon" : "Good evening";
        return `${moment}, ${this.session.name.split(/\s+/)[0]}`;
    }

    get today(): string {
        return new Date().toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
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

    /** Modules that open anything. */
    get modules(): readonly MenuEntry[] {
        return this.props.modules.filter((module) => actionsOf([module]).length > 0);
    }

    initial(module: MenuEntry): string {
        return module.name.trim().charAt(0).toUpperCase();
    }
}
