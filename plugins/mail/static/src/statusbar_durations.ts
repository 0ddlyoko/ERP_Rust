import { inject, load, props, resource } from "trame";
import { Orm } from "@web/core/orm";
import { widgetProps, widgets } from "@web/views/widgets/widget";
import { type Choice } from "@web/views/widgets/selection_widget";
import { StatusbarWidget } from "@web/views/widgets/statusbar_widget";

/** A change of the field, as `message.thread` describes it. */
interface Change {
    field: string;
    old_value: string | null;
    new_value: string | null;
}

interface Message {
    kind: string;
    date: string;
    changes: Change[];
}

/** A stretch of time the field held one value; `end` is null for the one it holds now. */
interface Period {
    key: string;
    start: Date | null;
    end: Date | null;
}

/**
 * A status bar that also says how long the record spent at each step, and since when it is at
 * the current one, from the changes its thread noted: `message.thread` asked for this field only.
 *
 * Works for any field holding an enum that is tracked. A record created before its creation was
 * noted starts at its creation date, if it has one, else at no known time.
 */
export class StatusbarDurationsWidget extends StatusbarWidget {
    static override template = "mail.StatusbarDurationsWidget";

    override props = props({ ...widgetProps });

    @inject(Orm) orm!: Orm;

    get recordId(): number | null {
        const id = this.props.record.id;
        return typeof id === "number" ? id : null;
    }

    @resource accessor history: { messages: Message[]; created: string | null } | null = load(
        () => ({ model: this.props.model, record: this.recordId, field: this.props.name, value: this.value }),
        async ({ model, record, field }) => {
            if (model === undefined || record === null) {
                return null;
            }
            const [messages, rows] = await Promise.all([
                this.orm.call<Message[]>("message", "thread", [], { model, record, field }),
                this.orm.read(model, [record], ["create_date"]),
            ]);
            return { messages, created: (rows[0]?.create_date as string | null | undefined) ?? null };
        },
    );

    /** What the field held and when, oldest first. */
    get periods(): Period[] {
        const history = this.history;
        if (history === null || history === undefined) {
            return [];
        }
        const changes = [...history.messages]
            .reverse()
            .flatMap((message) =>
                message.changes
                    .filter((change) => change.field === this.props.name)
                    .map((change) => ({ kind: message.kind, date: new Date(message.date), change })),
            );
        const created = history.created === null ? null : new Date(history.created);
        const first = changes[0];
        const periods: Period[] = [];
        let current: Period | null = null;
        if (first === undefined) {
            if (typeof this.value === "string") {
                current = { key: this.value, start: created, end: null };
            }
        } else if (first.kind !== "creation" && first.change.old_value !== null) {
            current = { key: first.change.old_value, start: created, end: null };
        }
        for (const { date, change } of changes) {
            if (current !== null) {
                current.end = date;
                periods.push(current);
            }
            current = change.new_value === null ? null : { key: change.new_value, start: date, end: null };
        }
        if (current !== null) {
            periods.push(current);
        }
        return periods;
    }

    /** When the record started at its first step, if known. */
    get since(): Date | null {
        return this.periods[0]?.start ?? null;
    }

    /** What a step says of the time spent there: dates and how long, for one passed or current. */
    timingOf(step: Choice): string {
        const visits = this.periods.filter((period) => period.key === step[0]);
        const last = visits.at(-1);
        if (last === undefined) {
            return "";
        }
        const spent = visits.reduce((total, period) => total + elapsed(period), 0);
        if (last.end === null) {
            const since = last.start === null ? "" : `Since ${dateText(last.start)} · `;
            return `${since}${durationText(spent)} · current`;
        }
        const from = last.start === null ? "…" : dateText(last.start);
        return `${from} → ${dateText(last.end)} · ${durationText(spent)}`;
    }

    get sinceText(): string {
        const since = this.since;
        return since === null ? "" : `Since creation: ${durationText(Date.now() - since.getTime())}`;
    }
}

function elapsed(period: Period): number {
    if (period.start === null) {
        return 0;
    }
    return (period.end ?? new Date()).getTime() - period.start.getTime();
}

/** `28/09 15:51`. */
function dateText(date: Date): string {
    const day = date.toLocaleDateString(undefined, { day: "2-digit", month: "2-digit" });
    const time = date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
    return `${day} ${time}`;
}

/** `3 d 21 h`, `2 h 5 min`, `12 min`, `< 1 min`. */
function durationText(milliseconds: number): string {
    const minutes = Math.floor(milliseconds / 60_000);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    if (days > 0) {
        return `${days} d ${hours % 24} h`;
    }
    if (hours > 0) {
        return `${hours} h ${minutes % 60} min`;
    }
    return minutes > 0 ? `${minutes} min` : "< 1 min";
}

widgets.add("statusbar_durations", StatusbarDurationsWidget);
