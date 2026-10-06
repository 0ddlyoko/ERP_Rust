import { state } from "trame";

/**
 * Where the user is: the action open, the kind of its view shown, the record, if one, and the
 * entry of the menu it was opened from — an action two modules share shows under the one chosen.
 */
export interface Route {
    action: string | null;
    view: string | null;
    id: number | null;
    menu?: number | null;
}

/** A route as the address writes it: `#action=base.action_users&view=form&id=3`. */
export function readRoute(hash: string): Route {
    const params = new URLSearchParams(hash.replace(/^#/, ""));
    const id = Number(params.get("id"));
    const menu = Number(params.get("menu"));
    return {
        action: params.get("action"),
        view: params.get("view"),
        id: Number.isInteger(id) && id > 0 ? id : null,
        menu: Number.isInteger(menu) && menu > 0 ? menu : null,
    };
}

export function writeRoute(route: Route): string {
    const params = new URLSearchParams();
    if (route.action !== null) {
        params.set("action", route.action);
    }
    if (route.view !== null) {
        params.set("view", route.view);
    }
    if (route.id !== null) {
        params.set("id", String(route.id));
    }
    if (route.menu !== null && route.menu !== undefined) {
        params.set("menu", String(route.menu));
    }
    return `#${params.toString()}`;
}

/**
 * The route of the page, kept in its address: reloading it, going back or following a link
 * shows the same thing.
 *
 * A guard may hold the user back — a form saves its changes first, and stays when it cannot: the
 * route stays, and the address goes back to it when the browser had already changed it.
 */
export class Router {
    @state accessor route: Route = readRoute(window.location.hash);

    /** Whether the user may leave the route, once it has done what it must: asked before leaving. */
    guard: (() => Promise<boolean>) | null = null;

    constructor() {
        window.addEventListener("hashchange", () => {
            const asked = readRoute(window.location.hash);
            if (writeRoute(asked) === writeRoute(this.route)) {
                return;
            }
            window.history.replaceState(null, "", writeRoute(this.route));
            void this.go(asked);
        });
        window.addEventListener("beforeunload", (event) => {
            if (this.guard !== null) {
                event.preventDefault();
            }
        });
    }

    /** Go somewhere: the address changes, and with it the route, unless the guard holds back. */
    async go(route: Route): Promise<void> {
        if (writeRoute(route) === writeRoute(this.route)) {
            return;
        }
        if (this.guard !== null && !(await this.guard())) {
            return;
        }
        this.route = route;
        window.history.pushState(null, "", writeRoute(route));
    }
}
