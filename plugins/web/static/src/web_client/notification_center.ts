import { Component, inject } from "trame";
import { type Notification, Notifications, SHOWN_FOR } from "@web/core/notifications";

/**
 * The notifications, one under the other in the top right corner, read out as they come.
 *
 * Under one that goes after a while, a bar empties as its time runs out; pointing at it holds it,
 * the bar full, until the pointer leaves.
 */
export class NotificationCenter extends Component {
    static template = "web.NotificationCenter";

    @inject(Notifications) notifications!: Notifications;

    get shown(): readonly Notification[] {
        return this.notifications.shown;
    }

    readonly countdown = `animation-duration: ${SHOWN_FOR}ms`;

    dismiss(notification: Notification): void {
        this.notifications.remove(notification.id);
    }

    classOf(notification: Notification): string {
        return `o_notification o_notification_${notification.kind}${notification.leaving ? " leaving" : ""}`;
    }
}
