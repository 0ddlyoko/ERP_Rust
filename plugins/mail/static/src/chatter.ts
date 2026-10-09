import { Component, effect, inject, load, loading, nextTick, props, resource, state, t } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { Notifications } from "@web/core/notifications";
import { Orm } from "@web/core/orm";
import { Session } from "@web/core/session";
import { formParts } from "@web/views/form/form_view";
import { SidePlace } from "@web/views/form/side_place";
import { RecordSearch } from "@web/views/widgets/record_search";
import { MessageFocus } from "./message_focus";

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
    /** The mails it was sent as, to those outside the application. */
    mails: { recipient: string | null; email: string | null; state: "outgoing" | "sent" | "failed"; error: string | null }[];
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

/** How many messages are read at a time, the next ones once nearly all are seen. */
const PAGE = 40;
/** How many messages left unseen below bring the next ones. */
const AHEAD = 5;

/** What the thread shows: everything, or only messages, notes or changes. */
type Shown = "all" | "message" | "note" | "change";

const KINDS: Record<Shown, string[] | null> = {
    all: null,
    message: ["comment"],
    note: ["note"],
    change: ["tracking", "creation"],
};

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
 * Read again each time the form reads its record, so a save shows what it changed. Read
 * forty messages at a time, the next forty once only five are left below; narrowed to messages,
 * notes or changes. Beside the form it scrolls within its column; the user may put it under the
 * form instead, which a narrow screen does on its own.
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
    @inject(MessageFocus) focus!: MessageFocus;
    @inject(Session) session!: Session;
    @inject(SidePlace) sidePlace!: SidePlace;

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

    @state accessor shown: Shown = "all";
    /** The message whose mails are listed, and where the list stands on the page. */
    @state accessor mailsOf: number | null = null;
    @state accessor mailsPlace = "";
    /** The messages read past the first page. */
    @state accessor older: Message[] = [];
    /** Whether the whole thread is read. */
    @state accessor exhausted = false;
    @state accessor loadingMore = false;

    /** The message just brought into sight, marked a moment. */
    @state accessor flashed: number | null = null;

    /** The composer's text box, set by its template. */
    box: HTMLTextAreaElement | null = null;
    /** The chatter's element, set by its template. */
    element: HTMLElement | null = null;
    /** The message the thread was read again for, once. */
    private reread: number | null = null;

    @resource accessor messages: Message[] = load(
        () => ({ model: this.props.model, record: this.props.record, version: this.props.version, changed: this.changed, shown: this.shown }),
        async ({ model, record, shown }) => {
            this.older = [];
            if (record === null) {
                this.exhausted = true;
                return [];
            }
            const first = await this.orm.call<Message[]>("message", "thread", [], { model, record, kinds: KINDS[shown], limit: PAGE });
            this.exhausted = first.length < PAGE;
            return first;
        },
    );

    /** The messages read so far, newest first. */
    get thread(): Message[] {
        return [...(this.messages ?? []), ...this.older];
    }

    show(shown: Shown): void {
        this.shown = shown;
    }

    /** Read the next messages, past those read. */
    async loadMore(): Promise<void> {
        if (this.exhausted || this.loadingMore || this.props.record === null || loading(() => this.messages)) {
            return;
        }
        this.loadingMore = true;
        try {
            const next = await this.orm.call<Message[]>("message", "thread", [], {
                model: this.props.model,
                record: this.props.record,
                kinds: KINDS[this.shown],
                offset: this.thread.length,
                limit: PAGE,
            });
            const known = new Set(this.thread.map((message) => message.id));
            this.older = [...this.older, ...next.filter((message) => !known.has(message.id))];
            this.exhausted = next.length < PAGE;
        } finally {
            this.loadingMore = false;
        }
    }

    /** Once the fifth message from the last read comes into sight, the next ones are read. */
    @effect readAhead(): (() => void) | void {
        const count = this.thread.length;
        if (this.exhausted || count === 0) {
            return;
        }
        let observer: IntersectionObserver | null = null;
        void nextTick().then(() => {
            const items = this.element?.querySelectorAll(".o_chatter_message");
            const watched = items?.[Math.max(0, count - AHEAD)];
            if (!watched) {
                return;
            }
            observer = new IntersectionObserver((entries) => {
                if (entries.some((entry) => entry.isIntersecting)) {
                    void this.loadMore();
                }
            });
            observer.observe(watched);
        });
        return () => observer?.disconnect();
    }

    /** Put the side column under the form, or back beside it. */
    toggleBelow(): void {
        void this.sidePlace.toggle();
    }

    @resource accessor info: Following | null = load(
        () => ({ model: this.props.model, record: this.props.record, version: this.props.version, changed: this.changed }),
        ({ model, record }) =>
            record === null ? Promise.resolve(null) : this.orm.call<Following>("follower", "of", [], { model, record }),
    );

    /**
     * The message opened from the inbox, once in the thread: scrolled to and marked a moment. Not
     * read yet, the thread is read again for it.
     */
    @effect bringFocused(): void {
        const focus = this.focus.wanted;
        if (focus === null || focus.model !== this.props.model || focus.record !== this.props.record || loading(() => this.messages)) {
            return;
        }
        const wanted = focus.message;
        if (!this.thread.some((message) => message.id === wanted) && this.reread !== wanted) {
            this.reread = wanted;
            this.changed++;
            return;
        }
        this.focus.wanted = null;
        this.flashed = wanted;
        void nextTick().then(() =>
            this.element?.querySelector(`[data-message="${wanted}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" }),
        );
        setTimeout(() => {
            if (this.flashed === wanted) {
                this.flashed = null;
            }
        }, 2400);
    }

    /** Whether the user wrote a message: shown on their side, to tell it from the others'. */
    isMine(message: Message): boolean {
        return message.author?.[0] === this.session.uid;
    }

    /** Where the mails of a message stand, in a word: all sent, some waiting, some failed. */
    mailState(message: Message): "sent" | "outgoing" | "failed" {
        if (message.mails.some((mail) => mail.state === "failed")) {
            return "failed";
        }
        return message.mails.some((mail) => mail.state === "outgoing") ? "outgoing" : "sent";
    }

    /** List a message's mails under its envelope, over the page: the thread scrolling would clip them. */
    toggleMails(message: Message, event: MouseEvent): void {
        const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
        this.mailsPlace = `top: ${box.bottom + 4}px; left: ${Math.max(8, Math.min(box.left, window.innerWidth - 300))}px`;
        this.mailsOf = this.mailsOf === message.id ? null : message.id;
    }

    /** Whether a message mentions the user. */
    mentionsMe(message: Message): boolean {
        const me = this.info?.me;
        return me !== null && me !== undefined && message.mentions.some(([id]) => id === me);
    }

    /** Open the followers, or close them — what each follows folded either way. */
    toggleFollowers(): void {
        this.followersOpen = !this.followersOpen;
        this.choosing = null;
    }

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
