import { Component, computed, inject, load, resource, state } from "trame";
import { type ActionDescription, actionsOf, type MenuEntry, Menus, moduleOf } from "@web/core/menus";
import { Orm } from "@web/core/orm";
import { type Route, Router } from "@web/core/router";
import { ActionManager } from "./action_manager";
import { LoadingIndicator } from "./loading_indicator";
import { NotificationCenter } from "./notification_center";
import { Sidebar } from "./sidebar";

/**
 * The root of the back office: the menu on the left, and the action open beside it.
 *
 * The action open is the route's: one of the menus', or, for one no menu leads to — a button
 * opening it — loaded by its identifier. With none, the first action of the menus. The module
 * shown is the one the action is under, unless the user switched to another.
 */
export class WebClient extends Component {
    static template = "web.WebClient";
    static components = { ActionManager, LoadingIndicator, NotificationCenter, Sidebar };

    @inject(Menus) menus!: Menus;
    @inject(Orm) orm!: Orm;
    @inject(Router) router!: Router;

    @state accessor switched: MenuEntry | null = null;

    get tree(): MenuEntry[] {
        return this.menus.tree;
    }

    get route(): Route {
        return this.router.route;
    }

    /** The action the route names, when no menu leads to it. */
    @resource accessor elsewhere: ActionDescription | null = load(
        () => {
            const asked = this.route.action;
            return asked !== null && this.fromMenus(asked) === undefined ? asked : null;
        },
        (asked) => (asked === null ? null : this.orm.call<ActionDescription>("action", "load", [], { xml_id: asked })),
    );

    private fromMenus(asked: string): ActionDescription | undefined {
        return actionsOf(this.tree ?? []).find((action) => action.xml_id === asked || String(action.id) === asked);
    }

    @computed get action(): ActionDescription | null {
        const asked = this.route.action;
        if (asked !== null) {
            return this.fromMenus(asked) ?? this.elsewhere ?? null;
        }
        return actionsOf(this.tree ?? [])[0] ?? null;
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
        this.router.go({ action: action.xml_id ?? String(action.id), view: null, id: null });
    }
}
