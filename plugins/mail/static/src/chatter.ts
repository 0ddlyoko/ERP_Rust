import { Component, inject, load, nextTick, props, resource, state, t } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { Notifications } from "@web/core/notifications";
import { Orm } from "@web/core/orm";
import { formParts } from "@web/views/form/form_view";
import { RecordSearch } from "@web/views/widgets/record_search";

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
    kind: "comment" | "note" | "tracking" | "creation";
    date: string;
    author: [number, string] | null;
    body: string | null;
    changes: Change[];
    subtype: string | null;
    recipients: [number, string | null][];
    mentions: [number, string | null][];
}

/** Who follows the record and what of it, as `follower.of` describes them. */
interface Following {
    following: boolean;
    me: number | null;
    contact: [number, string] | null;
    followers: { id: number; contact: [number, string]; subtypes: number[] }[];
    subtypes: { id: number; name: string }[];
}

/** A contact a message may mention: a user, by the contact they are. */
interface Mentionable {
    contact: number;
    name: string;
}

/** A part of a message's text: as written, or a mention of someone. */
interface Segment {
    text: string;
    mention: boolean;
}

/**
 * A record's thread, newest first, and what to say in it: a message to its followers and the
 * contact it is about, or an internal note, either mentioning people with `@` — who are told of
 * it in their inbox. Above, who follows the record — each following its discussions and the
 * changes they chose — and whether the user does.
 *
 * Read again each time the form reads its record, so a save shows what it changed.
 */
export class Chatter extends Component {
    static template = "mail.Chatter";
    static components = { RecordSearch };

    props = props({
        model: t.string(),
        record: t.number().orNull(),
        version: t.any(),
    });

    @inject(Orm) orm!: Orm;
    @inject(Notifications) notifications!: Notifications;

    /** What is being written: a message, a note, or nothing yet. */
    @state accessor composing: "message" | "note" | null = null;
    @state accessor text = "";
    @state accessor mentioned: Mentionable[] = [];
    /** What follows the `@` being typed, while a mention is being chosen. */
    @state accessor mentionQuery: string | null = null;
    @state accessor mentionResults: Mentionable[] = [];
    @state accessor mentionAt = 0;
    @state accessor followersOpen = false;
    /** The follower whose subtypes are shown to be chosen. */
    @state accessor choosing: number | null = null;
    @state accessor sending = false;
    /** Bumped once something was said or followed, for the thread and followers to be read again. */
    @state accessor changed = 0;

    /** The composer's text box, set by its template. */
    box: HTMLTextAreaElement | null = null;

    @resource accessor messages: Message[] = load(
        () => ({ model: this.props.model, record: this.props.record, version: this.props.version, changed: this.changed }),
        ({ model, record }) =>
            record === null ? Promise.resolve([]) : this.orm.call<Message[]>("message", "thread", [], { model, record }),
    );

    @resource accessor info: Following | null = load(
        () => ({ model: this.props.model, record: this.props.record, version: this.props.version, changed: this.changed }),
        ({ model, record }) =>
            record === null ? Promise.resolve(null) : this.orm.call<Following>("follower", "of", [], { model, record }),
    );

    authorOf(message: Message): string {
        return message.author?.[1] ?? "System";
    }

    initialsOf(name: string): string {
        return initialsOf(name);
    }

    avatarOf(name: string): string {
        return avatarStyleOf(name);
    }

    /** `Today · 10:42`, `Yesterday · 16:05`, else `28/09 · 16:05`. */
    whenOf(message: Message): string {
        const date = new Date(message.date);
        const time = date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
        const now = new Date();
        const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
        if (sameDay(date, now)) {
            return `Today · ${time}`;
        }
        if (sameDay(date, yesterday)) {
            return `Yesterday · ${time}`;
        }
        return `${date.toLocaleDateString(undefined, { day: "2-digit", month: "2-digit" })} · ${time}`;
    }

    /** Whom a message was said to: `Acme, Claire`. */
    recipientsOf(message: Message): string {
        return message.recipients.map(([id, name]) => name ?? `#${id}`).join(", ");
    }

    /** A message's text, the people it mentions set apart. */
    segmentsOf(message: Message): Segment[] {
        const names = message.mentions
            .map(([, name]) => name)
            .filter((name): name is string => !!name)
            .sort((left, right) => right.length - left.length);
        const body = message.body ?? "";
        if (!names.length) {
            return [{ text: body, mention: false }];
        }
        const pattern = new RegExp(`(${names.map((name) => `@${escapeRegExp(name)}`).join("|")})`, "g");
        return body
            .split(pattern)
            .filter((part) => part !== "")
            .map((part) => ({ text: part, mention: names.some((name) => part === `@${name}`) }));
    }

    /** Who a message goes to, said under the composer before it is sent. */
    get audience(): string {
        if (this.composing === "note") {
            return "Internal note: only those mentioned are told.";
        }
        const contact = this.info?.contact?.[1];
        const followers = this.info?.followers.length ? "the followers" : null;
        const told = [followers, contact].filter(Boolean).join(" and ");
        return told ? `To ${told}.` : "To the followers, once there are some.";
    }

    compose(kind: "message" | "note"): void {
        this.composing = this.composing === kind ? null : kind;
        void nextTick().then(() => this.box?.focus());
    }

    /** Typing: an `@` followed by letters looks for whom to mention. */
    type(event: Event): void {
        const box = event.target as HTMLTextAreaElement;
        this.text = box.value;
        const before = box.value.slice(0, box.selectionStart ?? box.value.length);
        const found = /(?:^|\s)@([\p{L}\p{N}._-]*)$/u.exec(before);
        if (found === null) {
            this.mentionQuery = null;
            return;
        }
        this.mentionQuery = found[1];
        void this.searchMentions(found[1]);
    }

    /** The users matching what follows the `@`, as the contacts they are. */
    async searchMentions(query: string): Promise<void> {
        const found = await this.orm.nameSearch("users", query, 6);
        if (this.mentionQuery !== query) {
            return;
        }
        const rows = found.length ? await this.orm.read("users", found.map(([id]) => id), ["contact"]) : [];
        this.mentionResults = rows
            .map((row) => ({ contact: row.contact as number, name: found.find(([id]) => id === row.id)?.[1] ?? "" }))
            .filter((one) => typeof one.contact === "number" && one.name !== "");
        this.mentionAt = 0;
    }

    /** Write the mention in place of the `@` typed, and keep whom it names. */
    mention(choice: Mentionable): void {
        const box = this.box;
        const caret = box?.selectionStart ?? this.text.length;
        const before = this.text.slice(0, caret).replace(/@[\p{L}\p{N}._-]*$/u, `@${choice.name} `);
        this.text = before + this.text.slice(caret);
        if (!this.mentioned.some((one) => one.contact === choice.contact)) {
            this.mentioned = [...this.mentioned, choice];
        }
        this.mentionQuery = null;
        void nextTick().then(() => {
            box?.focus();
            box?.setSelectionRange(before.length, before.length);
        });
    }

    key(event: KeyboardEvent): void {
        if (this.mentionQuery !== null && this.mentionResults.length) {
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                const by = event.key === "ArrowDown" ? 1 : -1;
                this.mentionAt = (this.mentionAt + by + this.mentionResults.length) % this.mentionResults.length;
                return;
            }
            if (event.key === "Enter" || event.key === "Tab") {
                event.preventDefault();
                this.mention(this.mentionResults[this.mentionAt]);
                return;
            }
            if (event.key === "Escape") {
                this.mentionQuery = null;
                return;
            }
        }
        if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            void this.send();
        }
    }

    /** Say what was written; those mentioned are those whose name it still holds. */
    async send(): Promise<void> {
        if (this.props.record === null || this.text.trim() === "" || this.sending) {
            return;
        }
        this.sending = true;
        try {
            await this.orm.call("message", "post", [], {
                model: this.props.model,
                record: this.props.record,
                body: this.text.trim(),
                internal: this.composing === "note",
                mentions: this.mentioned.filter((one) => this.text.includes(`@${one.name}`)).map((one) => one.contact),
            });
            this.text = "";
            this.mentioned = [];
            this.changed++;
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
        } finally {
            this.sending = false;
        }
    }

    /** Follow the record, or stop following it. */
    async toggleFollow(): Promise<void> {
        const method = this.info?.following ? "unfollow" : "follow";
        await this.act(() => this.orm.call("follower", method, [], { model: this.props.model, record: this.props.record }));
    }

    readonly addFollower = ([contact]: [number, string]): void => {
        void this.act(() =>
            this.orm.call("follower", "add", [], { model: this.props.model, record: this.props.record, contacts: [contact] }),
        );
    };

    removeFollower(follower: number): void {
        void this.act(() => this.orm.call("follower", "forget", [], { follower }));
    }

    /** Follow a subtype, or stop following it, for a follower. */
    toggleSubtype(follower: Following["followers"][number], subtype: number): void {
        const subtypes = follower.subtypes.includes(subtype)
            ? follower.subtypes.filter((id) => id !== subtype)
            : [...follower.subtypes, subtype];
        void this.act(() => this.orm.call("follower", "subscribe", [], { follower: follower.id, subtypes }));
    }

    /** The contacts following, not to be offered again. */
    get followerContacts(): number[] {
        return (this.info?.followers ?? []).map((follower) => follower.contact[0]);
    }

    private async act(work: () => Promise<unknown>): Promise<void> {
        try {
            await work();
            this.changed++;
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
        }
    }
}

function escapeRegExp(text: string): string {
    return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function sameDay(one: Date, other: Date): boolean {
    return (
        one.getFullYear() === other.getFullYear() &&
        one.getMonth() === other.getMonth() &&
        one.getDate() === other.getDate()
    );
}

formParts.add("chatter", Chatter);
