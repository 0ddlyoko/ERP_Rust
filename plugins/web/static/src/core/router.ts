import { state } from "trame";

/** Where the user is: the action open, the kind of its view shown, and the record, if one. */
export interface Route {
    action: string | null;
    view: string | null;
    id: number | null;
}

/** A route as the address writes it: `#action=base.action_users&view=form&id=3`. */
export function readRoute(hash: string): Route {
    const params = new URLSearchParams(hash.replace(/^#/, ""));
    const id = Number(params.get("id"));
    return {
        action: params.get("action"),
        view: params.get("view"),
        id: Number.isInteger(id) && id > 0 ? id : null,
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
    return `#${params.toString()}`;
}

/**
 * The route of the page, kept in its address: reloading it, going back or following a link
 * shows the same thing.
 */
export class Router {
    @state accessor route: Route = readRoute(window.location.hash);

    constructor() {
        window.addEventListener("hashchange", () => {
            this.route = readRoute(window.location.hash);
        });
    }

    /** Go somewhere: the address changes, and with it the route. */
    go(route: Route): void {
        window.location.hash = writeRoute(route);
    }
}
