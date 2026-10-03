import { state } from "trame";

/** A message shown for a while in a corner of the page. */
export interface Notification {
    id: number;
    kind: "info" | "success" | "danger";
    text: string;
}

/** How long a notification stays, in milliseconds, unless it is sticky. */
const SHOWN_FOR = 3000;

/** The notifications shown: what is happening, what went well, what went wrong. */
export class Notifications {
    @state accessor shown: Notification[] = [];

    private nextId = 1;

    /** Show a message; a sticky one stays until removed. Returns its id, to remove it. */
    add(kind: Notification["kind"], text: string, options: { sticky?: boolean } = {}): number {
        const id = this.nextId++;
        this.shown = [...this.shown, { id, kind, text }];
        if (!options.sticky) {
            setTimeout(() => this.remove(id), SHOWN_FOR);
        }
        return id;
    }

    remove(id: number): void {
        this.shown = this.shown.filter((notification) => notification.id !== id);
    }
}
