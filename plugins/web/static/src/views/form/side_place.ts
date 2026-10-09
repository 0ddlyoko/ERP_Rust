import { nextTick, state } from "trame";
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

    /** Move the column, the form and the column's cards sliding from where they were to where they go. */
    async toggle(): Promise<void> {
        const moving = Array.from(document.querySelectorAll<HTMLElement>(".o_form_main, .o_form_side > *"));
        const before = new Map(moving.map((element) => [element, element.getBoundingClientRect()]));
        this.below = !this.below;
        try {
            localStorage.setItem(BELOW_KEY, this.below ? "1" : "0");
        } catch {
            // Kept for this page only.
        }
        if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
            return;
        }
        await nextTick();
        for (const [element, was] of before) {
            const now = element.getBoundingClientRect();
            const dx = was.left - now.left;
            const dy = was.top - now.top;
            if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && Math.abs(was.width - now.width) < 1) {
                continue;
            }
            element.animate(
                [
                    { transform: `translate(${dx}px, ${dy}px)`, opacity: 0.35 },
                    { transform: "translate(0, 0)", opacity: 1 },
                ],
                { duration: 380, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
            );
        }
    }
}

services.add("web.side_place", SidePlace);
