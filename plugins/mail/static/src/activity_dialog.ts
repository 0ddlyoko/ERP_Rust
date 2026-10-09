import { Component, effect, inject, load, props, resource, state, t } from "trame";
import { Icon } from "@web/core/icons";
import { Notifications } from "@web/core/notifications";
import { Orm } from "@web/core/orm";
import { Session } from "@web/core/session";
import { RecordSearch } from "@web/views/widgets/record_search";

/** Something planned about a record, as `activity.of` and `activity.mine` describe it. */
export interface Planned {
    id: number;
    kind: { id: number; name: string; icon: string | null };
    summary: string | null;
    note: string | null;
    assignee: [number, string];
    deadline: string;
    model: string;
    record: number;
    record_name: string | null;
    planned_by: [number, string | null] | null;
    planned_on: string | null;
}

/** A kind of activity, as `activity.kinds` describes it. */
export interface ActivityKind {
    id: number;
    name: string;
    icon: string | null;
    delay: number;
}

/** A day as `YYYY-MM-DD`, `days` from today. */
export function dayFromToday(days: number): string {
    const date = new Date();
    date.setDate(date.getDate() + days);
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** When an activity is due, said from today, and how urgent: late, today, or ahead. */
export function dueOf(deadline: string): { text: string; state: "late" | "today" | "planned" } {
    const [year, month, day] = deadline.split("-").map(Number);
    const due = new Date(year, month - 1, day).getTime();
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    const days = Math.round((due - today) / 86_400_000);
    if (days < 0) {
        return { text: days === -1 ? "Yesterday" : `${-days} days late`, state: "late" };
    }
    if (days === 0) {
        return { text: "Today", state: "today" };
    }
    return { text: days === 1 ? "Tomorrow" : `In ${days} days`, state: "planned" };
}

/**
 * An activity in a dialog: what it is, about which record, for whom, by when, and who planned it.
 * From there it is changed, marked done with what came of it, or cancelled — and, from the inbox,
 * its record opened. Without an activity, one is planned about `model` and `record`: for the user,
 * by the usual delay of its kind, until they say otherwise.
 */
export class ActivityDialog extends Component {
    static template = "mail.ActivityDialog";
    static components = { Icon, RecordSearch };

    props = props({
        activity: t.any<Planned>().optional(),
        /** The record an activity is planned about, when there is none yet. */
        model: t.string().default(""),
        record: t.number().default(0),
        onClose: t.func<() => void>(),
        /** Called once the activity changed, was done or cancelled. */
        onChanged: t.func<() => void>(),
        /** Shown as a button when given: opens the record the activity is about. */
        onOpenRecord: t.func<() => void>().optional(),
    });

    @inject(Orm) orm!: Orm;
    @inject(Notifications) notifications!: Notifications;
    @inject(Session) session!: Session;

    /** The activity as last saved here, until the dialog closes. */
    @state accessor saved: Planned | null = null;
    @state accessor editing = this.props.activity === undefined;
    @state accessor finishing = false;
    @state accessor busy = false;
    @state accessor feedback = "";
    @state accessor kind: number | null = null;
    @state accessor summary = "";
    @state accessor note = "";
    @state accessor deadline = "";
    @state accessor assignee: [number, string] | null = this.props.activity === undefined ? [this.session.uid, this.session.name] : null;

    @resource accessor kinds: ActivityKind[] = load(() => this.orm.call<ActivityKind[]>("activity", "kinds", [], {}));

    /** Escape closes the dialog wherever the focus is, unless something in it took the key. */
    @effect closeOnEscape(): () => void {
        const close = (event: KeyboardEvent): void => {
            if (event.key === "Escape" && !event.defaultPrevented) {
                this.props.onClose();
            }
        };
        window.addEventListener("keydown", close);
        return () => window.removeEventListener("keydown", close);
    }

    /** Planning one, its kind is the first, once the kinds are read. */
    @effect firstKind(): void {
        if (this.creating && this.kind === null && this.kinds?.length) {
            this.pickKind(this.kinds[0]);
        }
    }

    get creating(): boolean {
        return this.props.activity === undefined;
    }

    get activity(): Planned | null {
        return this.saved ?? (this.props.activity as Planned | undefined) ?? null;
    }

    get due(): { text: string; state: string } {
        return dueOf(this.activity?.deadline ?? this.deadline);
    }

    /** The kind shown in the header: the one being chosen while planning. */
    get shownKind(): { name: string; icon: string | null } {
        if (this.creating) {
            const kind = this.kinds?.find((known) => known.id === this.kind);
            return { name: kind?.name ?? "Activity", icon: kind?.icon ?? "calendar" };
        }
        return this.activity?.kind ?? { name: "", icon: null };
    }

    /** Choose a kind; planning one, its deadline moves to the kind's usual delay. */
    pickKind(kind: ActivityKind): void {
        this.kind = kind.id;
        if (this.creating) {
            this.deadline = dayFromToday(kind.delay);
        }
    }

    /** A day as the user reads it: weekday, day, month and year. */
    longDay(day: string): string {
        const [year, month, date] = day.slice(0, 10).split("-").map(Number);
        return new Date(year, month - 1, date).toLocaleDateString(undefined, {
            weekday: "short",
            day: "numeric",
            month: "short",
            year: "numeric",
        });
    }

    startEdit(): void {
        const activity = this.activity;
        if (activity === null) {
            return;
        }
        this.kind = activity.kind.id;
        this.summary = activity.summary ?? "";
        this.note = activity.note ?? "";
        this.deadline = activity.deadline;
        this.assignee = activity.assignee;
        this.finishing = false;
        this.editing = true;
    }

    readonly pickAssignee = (choice: [number, string]): void => {
        this.assignee = choice;
    };

    /** Plan the activity, or save what was changed and show it. */
    async save(): Promise<void> {
        const kind = this.kinds?.find((known) => known.id === this.kind);
        const activity = this.activity;
        if (kind === undefined || this.assignee === null || !this.deadline) {
            return;
        }
        const values = { kind: kind.id, summary: this.summary, note: this.note, assignee: this.assignee[0], deadline: this.deadline };
        if (activity === null) {
            const planned = await this.act(() =>
                this.orm.call("activity", "schedule", [], { model: this.props.model, record: this.props.record, ...values }),
            );
            if (planned) {
                this.props.onClose();
            }
            return;
        }
        if (await this.act(() => this.orm.call("activity", "change", [], { activity: activity.id, ...values }))) {
            this.saved = {
                ...activity,
                kind: { id: kind.id, name: kind.name, icon: kind.icon },
                summary: this.summary.trim() ? this.summary : null,
                note: this.note.trim() ? this.note : null,
                assignee: this.assignee,
                deadline: this.deadline,
            };
            this.editing = false;
        }
    }

    /** Leave the changes: planning one, the dialog closes. */
    discard(): void {
        if (this.creating) {
            this.props.onClose();
            return;
        }
        this.editing = false;
    }

    startFinish(): void {
        this.feedback = "";
        this.editing = false;
        this.finishing = true;
    }

    /** Mark it done, with what came of it: the record's thread says so. */
    async finish(): Promise<void> {
        const id = this.activity?.id;
        if (id !== undefined && (await this.act(() => this.orm.call("activity", "done", [], { activity: id, feedback: this.feedback })))) {
            this.props.onClose();
        }
    }

    async cancel(): Promise<void> {
        const id = this.activity?.id;
        if (id !== undefined && (await this.act(() => this.orm.call("activity", "cancel", [], { activity: id })))) {
            this.props.onClose();
        }
    }

    private async act(work: () => Promise<unknown>): Promise<boolean> {
        this.busy = true;
        try {
            await work();
            this.props.onChanged();
            return true;
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
            return false;
        } finally {
            this.busy = false;
        }
    }
}
