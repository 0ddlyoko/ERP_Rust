import { Component, computed, inject, state } from "trame";
import { type ActionDescription, actionsOf, type MenuEntry, Menus, moduleOf } from "../core/menus";
import { ActionManager } from "./action_manager";
import { Sidebar } from "./sidebar";

/**
 * The root of the back office: the menu on the left, and the action open beside it.
 *
 * The action open is written in the address, `#action=base.action_users`, so that reloading the
 * page or following a link opens it again; with none there, the first action of the menus. The
 * module shown is the one the action is under, unless the user switched to another.
 */
export class WebClient extends Component {
    static template = "web.WebClient";
    static components = { ActionManager, Sidebar };

    @inject(Menus) menus!: Menus;

    @state accessor chosen: ActionDescription | null = null;
    @state accessor switched: MenuEntry | null = null;

    get tree(): MenuEntry[] {
        return this.menus.tree;
    }

    @computed get action(): ActionDescription | null {
        if (this.chosen !== null) {
            return this.chosen;
        }
        const actions = actionsOf(this.tree ?? []);
        const asked = new URLSearchParams(window.location.hash.slice(1)).get("action");
        return actions.find((action) => action.xml_id === asked || String(action.id) === asked) ?? actions[0] ?? null;
    }

    @computed get module(): MenuEntry | null {
        const action = this.action;
        return this.switched ?? (action ? moduleOf(this.tree ?? [], action.id) : null) ?? this.tree?.[0] ?? null;
    }

    /** Show a module's menus, and open its first action. */
    switchTo = (module: MenuEntry): void => {
        this.switched = module;
        const first = actionsOf([module])[0];
        if (first !== undefined) {
            this.show(first);
        }
    };

    /** Open an entry's action, from the module it is under. */
    open = (entry: MenuEntry): void => {
        if (entry.action !== null) {
            this.switched = moduleOf(this.tree ?? [], entry.action.id);
            this.show(entry.action);
        }
    };

    private show(action: ActionDescription): void {
        this.chosen = action;
        window.location.hash = `action=${action.xml_id ?? action.id}`;
    }
}
