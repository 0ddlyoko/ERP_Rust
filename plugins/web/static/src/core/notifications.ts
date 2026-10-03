import { state } from "trame";

/** A message shown for a while in a corner of the page. */
export interface Notification {
    id: number;
    kind: "info" | "success" | "danger";
    text: string;
    /** Stays until removed, rather than going after a while. */
    sticky: boolean;
    /** Pointed at: its countdown waits, full, until it no longer is. */
    held: boolean;
    /** Which countdown runs: a new one each time it starts again, for what shows it to restart. */
    run: number;
    /** Fading out, before it is removed. */
    leaving: boolean;
}

/** How long a notification stays, in milliseconds, unless it is sticky or pointed at. */
export const SHOWN_FOR = 3000;

/** How long it takes to fade out: what the stylesheet's transition lasts. */
const FADING_FOR = 200;

/**
 * The notifications shown: what is happening, what went well, what went wrong.
 *
 * One that is not sticky goes after a while. Pointing at it stops the count and starts it over
 * once the pointer leaves, so it cannot go while being read. It fades out before it is removed.
 */
export class Notifications {
    @state accessor shown: Notification[] = [];

    private nextId = 1;
    private readonly timers = new Map<number, ReturnType<typeof setTimeout>>();

    /** Show a message; a sticky one stays until removed. Returns its id, to remove it. */
    add(kind: Notification["kind"], text: string, options: { sticky?: boolean } = {}): number {
        const id = this.nextId++;
        const sticky = options.sticky ?? false;
        this.shown = [...this.shown, { id, kind, text, sticky, held: false, run: 0, leaving: false }];
        if (!sticky) {
            this.countDown(id);
        }
        return id;
    }

    /** Stop the count while the notification is pointed at. */
    hold(id: number): void {
        if (!this.timers.has(id)) {
            return;
        }
        clearTimeout(this.timers.get(id));
        this.update(id, { held: true });
    }

    /** Count again, from the start, once it is no longer pointed at. */
    release(id: number): void {
        const notification = this.shown.find((shown) => shown.id === id);
        if (notification?.held) {
            this.countDown(id);
        }
    }

    /** Fade the notification out, then remove it. */
    remove(id: number): void {
        clearTimeout(this.timers.get(id));
        this.timers.delete(id);
        this.update(id, { leaving: true });
        setTimeout(() => {
            this.shown = this.shown.filter((notification) => notification.id !== id);
        }, FADING_FOR);
    }

    private countDown(id: number): void {
        const run = (this.shown.find((shown) => shown.id === id)?.run ?? 0) + 1;
        this.update(id, { held: false, run });
        this.timers.set(
            id,
            setTimeout(() => this.remove(id), SHOWN_FOR),
        );
    }

    private update(id: number, change: Partial<Notification>): void {
        this.shown = this.shown.map((notification) =>
            notification.id === id ? { ...notification, ...change } : notification,
        );
    }
}
