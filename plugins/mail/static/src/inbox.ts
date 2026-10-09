import { Component, effect, inject, loading, props, state, t } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { actionFor, Menus } from "@web/core/menus";
import { Notifications } from "@web/core/notifications";
import { Orm } from "@web/core/orm";
import { Router } from "@web/core/router";
import { systray } from "@web/web_client/systray";
import { ActivityDialog, dueOf, type Planned } from "./activity_dialog";
import { MessageFocus } from "./message_focus";

/** A message the user was told of, as `notification.inbox` describes it. */
interface Notice {
    id: number;
    read: boolean;
    reason: "follower" | "mention";
    model: string;
    record: number;
    record_name: string | null;
    message: {
        id: number;
        kind: string;
        author: [number, string] | null;
        body: string | null;
        subtype: string | null;
        date: string;
        changes: { label: string; new: string | null }[];
    };
}

/** How often the number of unread messages is asked again, in milliseconds. */
const POLL_EVERY = 60_000;

/**
 * The user's inbox, under the menu: how many messages they were told of and have not read —
 * asked again every minute and when the page comes back to the front — and, opened, those
 * messages, each opening its record and read from then on.
 */
export class InboxTray extends Component {
    static template = "mail.InboxTray";
    static components = { ActivityDialog };

    props = props({
        /** Whether the menu shows its names. */
        wide: t.boolean().default(true),
    });

    @inject(Orm) orm!: Orm;
    @inject(Router) router!: Router;
    @inject(Menus) menus!: Menus;
    @inject(Notifications) notifications!: Notifications;
    @inject(MessageFocus) focus!: MessageFocus;

    @state accessor unread = 0;
    @state accessor open = false;
    @state accessor notices: Notice[] | null = null;
    /** The user's activities, soonest first. */
    @state accessor activities: Planned[] = [];
    /** The activity shown in a dialog. */
    @state accessor opened: Planned | null = null;
    /** Where the open inbox stands on the page: beside the menu, level with its button. */
    @state accessor place = "";

    @effect poll(): () => void {
        const count = (): void => void this.count();
        count();
        const timer = setInterval(count, POLL_EVERY);
        window.addEventListener("focus", count);
        return () => {
            clearInterval(timer);
            window.removeEventListener("focus", count);
        };
    }

    async count(): Promise<void> {
        try {
            const [unread, due] = await Promise.all([
                this.orm.call<number>("notification", "unread", [], {}),
                this.orm.call<number>("activity", "due", [], {}),
            ]);
            this.unread = unread + due;
        } catch {
            // Asked again in a minute.
        }
    }

    async toggle(event: MouseEvent): Promise<void> {
        const box = (event.currentTarget as HTMLElement).closest(".o_sidebar")?.getBoundingClientRect();
        const button = (event.currentTarget as HTMLElement).getBoundingClientRect();
        this.place = `left: ${(box?.right ?? button.right) + 12}px; bottom: ${Math.max(12, window.innerHeight - button.bottom)}px`;
        this.open = !this.open;
        if (this.open) {
            [this.notices, this.activities] = await Promise.all([
                this.orm.call<Notice[]>("notification", "inbox", [], { limit: 30 }),
                this.orm.call<Planned[]>("activity", "mine", [], { limit: 20 }),
            ]);
            await this.count();
        }
    }

    /** What a notice says in a line: the message, or the change it notes. */
    summaryOf(notice: Notice): string {
        const message = notice.message;
        if (message.body) {
            return message.body;
        }
        const changes = message.changes.map((change) => `${change.label}: ${change.new ?? "—"}`).join(", ");
        return changes || (message.subtype ?? "");
    }

    authorOf(notice: Notice): string {
        return notice.message.author?.[1] ?? "System";
    }

    initialsOf(name: string): string {
        return initialsOf(name);
    }

    avatarOf(name: string): string {
        return avatarStyleOf(name);
    }

    dueOf(deadline: string): { text: string; state: string } {
        return dueOf(deadline);
    }

    /** Show an activity in a dialog, the inbox closed. */
    openActivity(activity: Planned): void {
        this.open = false;
        this.opened = activity;
    }

    readonly closeActivity = (): void => {
        this.opened = null;
    };

    readonly activityChanged = (): void => {
        void this.count();
    };

    /** Open the record the activity shown is about. */
    readonly openActivityRecord = async (): Promise<void> => {
        const activity = this.opened;
        this.opened = null;
        if (activity === null) {
            return;
        }
        const tree = loading(() => this.menus.tree) ? [] : (this.menus.tree ?? []);
        const action = actionFor(tree, activity.model);
        if (action !== null) {
            await this.router.go({ action, view: "form", id: activity.record });
        }
    };

    /** Open the record a notice is about, at its message, the notice read from then on. */
    async openNotice(notice: Notice): Promise<void> {
        await this.markRead([notice.id]);
        const tree = loading(() => this.menus.tree) ? [] : (this.menus.tree ?? []);
        const action = actionFor(tree, notice.model);
        this.open = false;
        if (action === null) {
            this.notifications.add("warning", `No menu opens ${notice.record_name ?? notice.model}.`);
            return;
        }
        this.focus.wanted = { model: notice.model, record: notice.record, message: notice.message.id };
        await this.router.go({ action, view: "form", id: notice.record });
    }

    async markRead(notifications: number[]): Promise<void> {
        await this.orm.call("notification", "mark_read", [], { notifications });
        this.notices = (this.notices ?? []).map((notice) =>
            notifications.length === 0 || notifications.includes(notice.id) ? { ...notice, read: true } : notice,
        );
        await this.count();
    }
}

systray.add("mail.inbox", InboxTray);
