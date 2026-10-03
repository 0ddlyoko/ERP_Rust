import { Component, inject, load, props, resource, t } from "trame";
import { Orm } from "@web/core/orm";
import { formParts } from "@web/views/form/form_view";

/** A tracked field a message notes the change of, as text. */
interface Change {
    field: string;
    label: string;
    old: string | null;
    new: string | null;
}

/** A message of a record's thread, as `message.thread` describes it. */
interface Message {
    id: number;
    kind: "comment" | "note" | "tracking";
    date: string;
    author: [number, string] | null;
    body: string | null;
    changes: Change[];
}

/**
 * A record's thread, newest first: who changed which tracked field, from what to what, and when.
 *
 * Read again each time the form reads its record, so a save shows what it changed.
 */
export class Chatter extends Component {
    static template = "mail.Chatter";

    props = props({
        model: t.string(),
        record: t.number().orNull(),
        version: t.any(),
    });

    @inject(Orm) orm!: Orm;

    @resource accessor messages: Message[] = load(
        () => ({ model: this.props.model, record: this.props.record, version: this.props.version }),
        ({ model, record }) =>
            record === null ? Promise.resolve([]) : this.orm.call<Message[]>("message", "thread", [], { model, record }),
    );

    authorOf(message: Message): string {
        return message.author?.[1] ?? "System";
    }

    /** Up to two initials of the author: `Nathan Giacomello` reads `NG`. */
    initialsOf(message: Message): string {
        return this.authorOf(message)
            .split(/\s+/)
            .filter((word) => word.length > 0)
            .slice(0, 2)
            .map((word) => word[0].toUpperCase())
            .join("");
    }

    /** `Today · 10:42`, `Yesterday · 16:05`, else `28/09 · 16:05`. */
    whenOf(message: Message): string {
        const date = new Date(message.date);
        const time = date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
        const day = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
        const now = new Date();
        const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
        if (day === today) {
            return `Today · ${time}`;
        }
        if (day === today - 86_400_000) {
            return `Yesterday · ${time}`;
        }
        return `${date.toLocaleDateString(undefined, { day: "2-digit", month: "2-digit" })} · ${time}`;
    }
}

formParts.add("chatter", Chatter);
