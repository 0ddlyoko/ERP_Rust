import { state } from "trame";
import { services } from "@web/web_client/systray";

/** Where the browser remembers the side column put under the form. */
const BELOW_KEY = "o_form_side_below";

function readBelow(): boolean {
    try {
        return localStorage.getItem(BELOW_KEY) === "1";
    } catch {
        return false;
    }
}

/**
 * Where a form's side column — its progress, its thread — stands: beside the form, the thread
 * scrolling within it, or under the form across its width, for the form to take the whole width.
 * Chosen by the user, kept by the browser; a narrow screen puts it under the form whatever is
 * chosen.
 */
export class SidePlace {
    @state accessor below = readBelow();

    toggle(): void {
        this.below = !this.below;
        try {
            localStorage.setItem(BELOW_KEY, this.below ? "1" : "0");
        } catch {
            // Kept for this page only.
        }
    }
}

services.add("web.side_place", SidePlace);
