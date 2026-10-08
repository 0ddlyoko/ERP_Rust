import { inject, state } from "trame";
import { listKey, type Route, Router, writeRoute } from "./router";

/**
 * A record or a list the user left for another view: where it was, under which action, its name,
 * and the list beside it then.
 */
export interface Crumb {
    route: Route;
    action: string;
    name: string;
    strip: Route | null;
}

/** The list a route is in: its action, narrowed as it is, in the view of several it shows — list or board — no record. */
export function listOf(route: Route): Route {
    const view = route.view === "form" ? null : route.view;
    return { action: route.action, view, id: null, ids: route.ids ?? null, by: route.by ?? null, menu: route.menu };
}

/**
 * What the breadcrumb shows: the records and lists left on the way to the view open — opening the
 * record of a many2one, the records of a related link — then the action, and the record a view
 * has open, if one.
 *
 * It also holds the list shown beside a record, `strip`: the one the user last opened a record
 * from. Opening another record from that one — a customer, a delivery — keeps it; opening several
 * shows them as a list, beside it; opening one of those makes theirs the list beside. Going back
 * to a crumb brings back the list that was beside it.
 *
 * The trail changes once the next view shows, not when the route does: the view left stays shown
 * while the next one loads, and the breadcrumb with it.
 */
export class Breadcrumb {
    @inject(Router) router!: Router;

    @state accessor trail: Crumb[] = [];
    @state accessor action: string | null = null;
    @state accessor record: string | null = null;
    /** What the list shown is called: the project of a board, else its action. */
    @state accessor listName: string | null = null;
    /** What the last list shown was called, which a record opened from it names it by. */
    private lastListName: string | null = null;
    /** The list beside a record, or beside a list opened from one; that of the action when none. */
    @state accessor strip: Route | null = null;

    /** The record being left, added to the trail once the next one shows. */
    private leaving: Crumb | null = null;
    /** The crumb being gone back to, by position: the trail ends before it once its record shows. */
    private returning: number | null = null;
    /** A record chosen in the list beside: the trail goes back to that list. */
    private picking = false;
    /** The list shown last, while it is the view shown: a record opened next is opened from it. */
    private fromList: Route | null = null;
    /** The view shown last, by route and kind: shown again — laid out anew — it changes nothing. */
    private lastShown = "";

    /**
     * Go back to a crumb: those from it on are dropped once its record shows — at once when it is
     * the record already open, which the user may have come back to another way. By position, as
     * one record can stand in the trail more than once.
     */
    async back(index: number): Promise<void> {
        const crumb = this.trail[index];
        if (crumb === undefined) {
            return;
        }
        const target = writeRoute(crumb.route);
        if (target === writeRoute(this.router.route)) {
            this.trail = this.trail.slice(0, index);
            return;
        }
        this.returning = index;
        await this.router.go(crumb.route);
        if (writeRoute(this.router.route) !== target) {
            this.returning = null;
        }
    }

    /** Open a record of an action, the record open left in the trail once the user did leave it. */
    async open(action: string, id: number): Promise<void> {
        const here = this.router.route;
        const go = () => this.router.go({ action, view: "form", id });
        if (here.id === null) {
            await go();
            return;
        }
        await this.leave(here, go, () => writeRoute(this.router.route) !== writeRoute(here));
    }

    /** Name the list shown, `null` once a record shows instead. */
    nameList(name: string | null): void {
        this.listName = name;
        if (name !== null) {
            this.lastListName = name;
        }
    }

    /** Open some records of an action in its first view of several, what is open left in the trail. */
    async openList(action: string, ids: number[]): Promise<void> {
        const here = this.router.route;
        const go = () => this.router.go({ action, view: null, id: null, ids, menu: here.menu });
        await this.leave(here, go, () => writeRoute(this.router.route) !== writeRoute(here));
    }

    /** Open the records of an action belonging to one record — a project's tasks — likewise. */
    async openBy(action: string, field: string, id: number): Promise<void> {
        const here = this.router.route;
        const go = () => this.router.go({ action, view: null, id: null, by: `${field}:${id}`, menu: here.menu });
        await this.leave(here, go, () => writeRoute(this.router.route) !== writeRoute(here));
    }

    /** Open a record of the list beside, or a new one: the trail goes back to that list. */
    async pick(id: number | null): Promise<void> {
        const strip = this.strip ?? listOf(this.router.route);
        this.picking = true;
        await this.router.go({ ...strip, view: "form", id });
    }

    /** Leave the record open for another, through `go`; kept in the trail if it was left. */
    async leave(route: Route, go: () => Promise<void>, left: () => boolean): Promise<void> {
        const isList = route.id === null;
        this.leaving = {
            route,
            action: this.action ?? "",
            name: (isList ? this.listName : this.record) ?? this.action ?? "",
            strip: isList ? this.strip : (this.strip ?? listOf(route)),
        };
        await go();
        if (!left()) {
            this.leaving = null;
        }
    }

    /**
     * A view shows. Coming back to a crumb drops it and those after it, and brings back its list
     * beside. What was left joins the trail. A list opened otherwise — from the menu — forgets
     * them all; a record opened from a list has that list beside it, and the trail leads back to
     * the list when it was itself opened from a record, or is narrowed — a project's board.
     */
    shown(route: Route, isRecord: boolean): void {
        const shown = `${writeRoute(route)}/${isRecord}`;
        if (shown === this.lastShown && this.leaving === null && this.returning === null && !this.picking) {
            return;
        }
        this.lastShown = shown;
        const { leaving, returning, picking, fromList } = this;
        this.leaving = null;
        this.returning = null;
        this.picking = false;
        this.fromList = isRecord ? null : listOf(route);
        if (returning !== null) {
            this.strip = this.trail[returning]?.strip ?? null;
            this.trail = this.trail.slice(0, returning);
        } else if (leaving !== null) {
            this.trail = [...this.trail, leaving];
            this.strip = leaving.strip;
        } else if (!isRecord) {
            this.trail = [];
            this.strip = null;
        } else if (picking) {
            const strip = listKey(this.strip ?? listOf(route));
            const at = this.trail.findIndex((crumb) => crumb.route.id === null && listKey(crumb.route) === strip);
            this.trail = this.trail.slice(0, at + 1);
        } else if (fromList !== null) {
            if (this.trail.length || fromList.ids || fromList.by) {
                const name = this.lastListName ?? this.action ?? "";
                this.trail = [...this.trail, { route: fromList, action: this.action ?? "", name, strip: this.strip }];
            }
            this.strip = fromList;
        } else {
            const at = this.trail.findIndex((crumb) => writeRoute(crumb.route) === writeRoute(route));
            if (at >= 0) {
                this.strip = this.trail[at].strip;
                this.trail = this.trail.slice(0, at);
            }
        }
    }
}
