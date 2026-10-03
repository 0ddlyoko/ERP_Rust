import { state } from "trame";
import { type Route, writeRoute } from "./router";

/** A record the user left for another one: where it was, under which action, and its name. */
export interface Crumb {
    route: Route;
    action: string;
    name: string;
}

/**
 * What the breadcrumb shows: the records left on the way to the one open — opening the record of
 * a many2one — then the action, and the record a view has open, if one.
 *
 * The trail changes once the next view shows, not when the route does: the view left stays shown
 * while the next one loads, and the breadcrumb with it.
 */
export class Breadcrumb {
    @state accessor trail: Crumb[] = [];
    @state accessor action: string | null = null;
    @state accessor record: string | null = null;

    /** The record being left, added to the trail once the next one shows. */
    private leaving: Crumb | null = null;

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
        this.leaving = null;
        if (!isRecord) {
            this.trail = [];
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
