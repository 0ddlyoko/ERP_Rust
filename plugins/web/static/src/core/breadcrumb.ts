import { inject, state } from "trame";
import { type Route, Router, writeRoute } from "./router";

/** A record the user left for another one: where it was, under which action, and its name. */
export interface Crumb {
    route: Route;
    action: string;
    name: string;
}

/**
 * What the breadcrumb shows: the records left on the way to the one open — opening the record of
 * a many2one, or of a one2many or many2many — then the action, and the record a view has open, if
 * one.
 *
 * The trail changes once the next view shows, not when the route does: the view left stays shown
 * while the next one loads, and the breadcrumb with it.
 */
export class Breadcrumb {
    @inject(Router) router!: Router;

    @state accessor trail: Crumb[] = [];
    @state accessor action: string | null = null;
    @state accessor record: string | null = null;

    /** The record being left, added to the trail once the next one shows. */
    private leaving: Crumb | null = null;
    /** The crumb being gone back to, by position: the trail ends before it once its record shows. */
    private returning: number | null = null;

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

    /** Leave the record open for another, through `go`; kept in the trail if it was left. */
    async leave(route: Route, go: () => Promise<void>, left: () => boolean): Promise<void> {
        this.leaving = { route, action: this.action ?? "", name: this.record ?? "" };
        await go();
        if (!left()) {
            this.leaving = null;
        }
    }

    /**
     * A view shows: the record left joins the trail; coming back to a crumb drops it and those
     * after it; a list forgets them all.
     */
    shown(route: Route, isRecord: boolean): void {
        const leaving = this.leaving;
        const returning = this.returning;
        this.leaving = null;
        this.returning = null;
        if (!isRecord) {
            this.trail = [];
        } else if (returning !== null) {
            this.trail = this.trail.slice(0, returning);
        } else if (leaving !== null) {
            this.trail = [...this.trail, leaving];
        } else {
            const at = this.trail.findIndex((crumb) => writeRoute(crumb.route) === writeRoute(route));
            if (at >= 0) {
                this.trail = this.trail.slice(0, at);
            }
        }
    }
}
