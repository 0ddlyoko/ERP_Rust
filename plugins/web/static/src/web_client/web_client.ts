import { Component, computed, inject, load, resource, state } from "trame";
import { type ActionDescription, actionsOf, type MenuEntry, Menus, moduleOf, moduleOfModel } from "@web/core/menus";
import { Orm } from "@web/core/orm";
import { type Route, Router } from "@web/core/router";
import { ActionManager } from "./action_manager";
import { Home } from "./home";
import { LoadingIndicator } from "./loading_indicator";
import { NotificationCenter } from "./notification_center";
import { Sidebar } from "./sidebar";

/**
 * The root of the back office: the menu on the left, and the action open beside it.
 *
 * The action open is the route's: one of the menus', or, for one no menu leads to — a button
 * opening it — loaded by its identifier. With none, the home page. The module
 * shown is the one the action is under — for one no menu leads to, the one with an action on the
 * same model — unless the user switched to another.
 */
export class WebClient extends Component {
    static template = "web.WebClient";
    static components = { ActionManager, Home, LoadingIndicator, NotificationCenter, Sidebar };

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
        return null;
    }

    @computed get module(): MenuEntry | null {
        const action = this.action;
        const tree = this.tree ?? [];
        const under = action ? (moduleOf(tree, action.id) ?? moduleOfModel(tree, action.model)) : null;
        return this.switched ?? under ?? tree[0] ?? null;
    }

    /** Back to the home page. */
    goHome = (): void => {
        this.switched = null;
        this.router.go({ action: null, view: null, id: null });
    };

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
