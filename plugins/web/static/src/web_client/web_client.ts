import { Component, computed, inject, load, resource, state } from "trame";
import { type ActionDescription, actionsOf, type MenuEntry, Menus } from "../core/menus";
import { ActionManager } from "./action_manager";

/**
 * The root of the back office: the menus the user sees, and the action open.
 *
 * The action open is written in the address, `#action=base.action_users`, so that reloading the
 * page or following a link opens it again; with none there, the first action of the menus.
 */
export class WebClient extends Component {
    static template = "web.WebClient";
    static components = { ActionManager };

    @inject(Menus) menus!: Menus;

    @resource accessor tree: MenuEntry[] = load(() => this.menus.tree());

    @state accessor chosen: ActionDescription | null = null;
    @state accessor openEntry: MenuEntry | null = null;

    /** The top entry whose children the side shows: the one chosen, or the first. */
    @computed get app(): MenuEntry | null {
        return this.openEntry ?? this.tree?.[0] ?? null;
    }

    @computed get action(): ActionDescription | null {
        if (this.chosen !== null) {
            return this.chosen;
        }
        const actions = actionsOf(this.tree ?? []);
        const asked = new URLSearchParams(window.location.hash.slice(1)).get("action");
        return actions.find((action) => action.xml_id === asked || String(action.id) === asked) ?? actions[0] ?? null;
    }

    /** Open an entry: its action if it has one, and its children in the side if it is a top one. */
    choose(entry: MenuEntry, top = false): void {
        if (top) {
            this.openEntry = entry;
        }
        const action = entry.action ?? (top ? actionsOf(entry.children)[0] : undefined);
        if (action !== undefined) {
            this.chosen = action;
            window.location.hash = `action=${action.xml_id ?? action.id}`;
        }
    }
}
