import { Component, inject } from "trame";
import { type Notification, Notifications } from "@web/core/notifications";

/** The notifications, stacked in the top right corner and read out as they come. */
export class NotificationCenter extends Component {
    static template = "web.NotificationCenter";

    @inject(Notifications) notifications!: Notifications;

    get shown(): readonly Notification[] {
        return this.notifications.shown;
    }

    dismiss(notification: Notification): void {
        this.notifications.remove(notification.id);
    }
}
