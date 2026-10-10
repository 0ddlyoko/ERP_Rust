import { type ComponentClass, computed, effect, inject, load, loading, nextTick, props, registry, resource, state, t, untrack } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { listMemory } from "@web/core/list_memory";
import { listKey } from "@web/core/router";
import { Notifications } from "@web/core/notifications";
import type { Fields } from "@web/core/models";
import type { Values } from "@web/core/orm";
import { asksReload, opensRecord, View, viewKinds, viewProps } from "@web/views/view";
import { companionFields } from "@web/views/widgets/decimal_widget";
import { bodyFor } from "./form_body";
import { type CompiledForm, compileForm, type FormButton } from "./form_compiler";
import { SidePlace } from "./side_place";

/** How long changing has to pause before the fields computed from it are asked for, in milliseconds. */
const ONCHANGE_AFTER = 250;

/** How far down the page is scrolled when the leader shrinks, and how far up when it grows again. */
const LEADER_SHRINKS_PAST = 160;
const LEADER_GROWS_BEFORE = 40;
/** How much of its height the leader loses at most when it keeps to one line. */
const LEADER_SHRINKS_BY = 0.7;

/**
 * Parts of a form other plugins provide: `chatter`, the record's thread, takes `model`, `record`
 * (null until the record is created) and `version`, which changes each time the record is read.
 */
export const formParts = registry.category<ComponentClass>("form_parts");

/**
 * One record, its fields laid out as its view's XML says, edited in place.
 *
 * What the user changes is kept apart from what was read: saving writes only that, discarding
 * forgets it. A record not created yet starts from its fields' defaults, and saving creates it.
 * Labels may show a field's value — `string="Groups of {{ name }}"` — and follow it as it is
 * edited.
 *
 * It names its record in the breadcrumb, steps through the records of the list it was opened
 * from, and saves its changes before the user leaves it — staying when they cannot be saved.
 *
 * Its XML becomes a Trame template ([`compileForm`](./form_compiler.ts)), so `invisible`,
 * `readonly` and `required` are expressions Trame evaluates, reading the record's fields by name.
 */
export class FormView extends View {
    static template = "web.FormView";

    override props = props({
        ...viewProps,
        /** Called with a record once created, as `[id, name]`, instead of opening it. */
        onCreated: t.func<(record: [number, string | null]) => void>().optional(),
        /** Shown beside the records of its list, which the user steps through instead of a pager. */
        besideList: t.boolean().default(false),
        /**
         * An assistant in a dialog — a wizard: no bar to save, only the buttons of its `<footer>`,
         * which create the record and run their method on it, then close the dialog.
         */
        dialog: t.boolean().default(false),
        /** Called with what a footer button's method answered, once it ran. */
        onDone: t.func<(answer: unknown) => void>().optional(),
        /** Called to close the dialog the form is in. */
        onClose: t.func<() => void>().optional(),
    });

    @inject(Notifications) notifications!: Notifications;
    @inject(SidePlace) sidePlace!: SidePlace;

    override get kind(): string {
        return "form";
    }

    /**
     * A click on a field — its label, or beside its input — types in it, as a click on its input
     * would: a relation's then lists the records to choose from. A click on what reacts to it
     * itself — a button, a link, an input — is left to it.
     */
    focusField(event: MouseEvent): void {
        const target = event.target as HTMLElement;
        if (target.closest("input, textarea, select, button, a, [contenteditable], .o_record_search_results, .o_x2many_list")) {
            return;
        }
        const field = target.closest<HTMLElement>(".o_form_field, .o_leader_tile, .o_form_heading_field");
        const input = field?.querySelector<HTMLElement>(
            "input:not([type='hidden']):not([disabled]):not([readonly]), textarea:not([disabled]), select:not([disabled])",
        );
        input?.focus();
        if (input instanceof HTMLInputElement && input.type === "checkbox") {
            input.click();
        }
    }

    @state accessor changes: Values = {};
    @state accessor saving = false;
    /** The button whose method runs, by its position in the bar, until it is done. */
    @state accessor pressing: number | null = null;
    @state accessor failure: string | null = null;
    @state accessor openPages = new Map<number, number>();
    /** Whether a save was tried: required fields left empty are shown from then on. */
    @state accessor tried = false;
    /** What the server computed from what the user changed, shown over the record until saved. */
    @state accessor computedValues: Values = {};
    /** The same for the lines of its one2many and many2many: by field, then by line key. */
    @state accessor computedLines: Record<string, Record<string, Values>> = {};
    /** Why fields of the record could not be computed, by field. */
    @state accessor computeErrors: Record<string, string> = {};
    /** The same for its lines: by one2many or many2many, then line key, then field. */
    @state accessor lineErrors: Record<string, Record<string, Record<string, string>>> = {};
    /** Whether the leader shows one line: the page is scrolled past it. */
    @state accessor leaderCompact = false;
    /** What the user was told could not be computed, so as to tell it once. */
    private told = new Set<string>();
    private onchanges = 0;
    private onchangeTimer: ReturnType<typeof setTimeout> | undefined;

    /** What shows the record's thread, if a plugin provides it. */
    get chatter(): ComponentClass | null {
        return formParts.get("chatter", null);
    }

    /** The form's body, set by its template. */
    element: HTMLElement | null = null;

    /** The record as read, or what the server says one not created yet starts with. */
    @resource accessor record: Values = load(
        () => ({
            model: this.props.resModel,
            id: this.props.resId,
            names: this.readNames,
            fields: this.fields,
            defaults: this.props.defaults,
            version: this.orm.versionOf(this.props.resModel),
        }),
        async ({ model, id, names, defaults }) =>
            id === undefined
                ? { ...(await this.orm.defaultGet(model, names.filter((name) => name !== "id"))), ...defaults }
                : this.read(model, id, names),
    );

    private async read(model: string, id: number, names: string[]): Promise<Values> {
        const [record] = await this.orm.read(model, [id], names, { names: true });
        if (record === undefined) {
            throw new Error(`Record ${model} #${id} does not exist, or is not yours to read`);
        }
        return record;
    }

    /** The form's XML as a template, with what it refers to. */
    @computed get layout(): CompiledForm | undefined {
        const root = this.archRoot;
        const fields = this.fields;
        if (root === undefined || fields === undefined) {
            return undefined;
        }
        return compileForm(root, (element) => this.columnOf(element, fields));
    }

    @computed get body(): ComponentClass | null {
        return this.layout === undefined ? null : bodyFor(this.layout.source);
    }

    /**
     * The fields read with the record: those shown, those its related links count or show, and
     * those a condition reads.
     */
    @computed get readNames(): string[] {
        const fields = this.fields ?? {};
        const conditions = (this.layout?.conditionNames ?? []).filter((name) => name in fields);
        const related = [...(this.layout?.relatedFields ?? []).map(({ name }) => name), ...(this.layout?.figureFields ?? [])].filter(
            (name) => name in fields,
        );
        const companions = companionFields(this.columns, fields);
        return [...new Set([...this.columns.map((column) => column.name), ...related, ...conditions, ...companions])];
    }

    /** The record as the user sees it: what was read, what the server computed, what they changed. */
    @computed get current(): Values {
        return { ...this.record, ...this.computedValues, ...this.changes };
    }

    get isNew(): boolean {
        return this.props.resId === undefined;
    }

    get isDirty(): boolean {
        return Object.keys(this.changes).length > 0;
    }

    /** Saving, or running a button: what the user could press meanwhile waits. */
    get busy(): boolean {
        return this.saving || this.pressing !== null;
    }

    /** Something to save, and nothing under way. */
    get canSave(): boolean {
        return !this.busy && (this.isDirty || this.isNew);
    }

    /** A label as written, each `{{ field }}` replaced by the field's value; `\{{` is a brace. */
    label(text: string): string {
        return text.replace(/(\\)?\{\{\s*(\w+)\s*\}\}/g, (written, escaped: string | undefined, name: string) =>
            escaped ? written.slice(1) : this.display(name),
        );
    }

    /** A field's value as a label reads it: a record's name, how many records, a value's label, or as it is. */
    display(name: string): string {
        const value = this.current[name];
        if (value === null || value === undefined || value === false) {
            return "";
        }
        const values = this.fields?.[name]?.values;
        if (values !== undefined) {
            return values.find(([key]) => key === value)?.[1] ?? String(value);
        }
        if (Array.isArray(value)) {
            if (this.fields?.[name]?.type === "ref") {
                const [id, recordName] = value as [number, string | null];
                return recordName ?? `#${id}`;
            }
            return String(value.length);
        }
        return String(value);
    }

    /** A number as a figure reads it: grouped, with at most two decimals; any other value as displayed. */
    figure(name: string): string {
        const value = this.current[name];
        const type = this.fields?.[name]?.type;
        if ((type === "decimal" || type === "integer") && value !== null && value !== undefined && value !== "") {
            return Number(value).toLocaleString(undefined, { maximumFractionDigits: 2 });
        }
        return this.display(name);
    }

    /** The record's name, as the breadcrumb shows it. */
    get title(): string {
        if (this.isNew) {
            return "New";
        }
        if (loading(() => this.record) || loading(() => this.fields)) {
            return "";
        }
        return this.fields?.name !== undefined && this.display("name") ? this.display("name") : `#${this.props.resId}`;
    }

    @effect nameInBreadcrumb(): (() => void) | void {
        if (this.props.embedded) {
            return;
        }
        this.breadcrumb.record = this.title;
        return () => {
            this.breadcrumb.record = null;
        };
    }

    /** A new record shows at once what the server computes from what it starts with. */
    @effect computeNew(): void {
        if (this.isNew && this.record !== undefined && this.fields !== undefined) {
            untrack(() => void this.onchange());
        }
    }

    /** While changes are not saved, closing or reloading the page asks the browser to confirm. */
    @effect guardUnload(): (() => void) | void {
        if (!this.isDirty || this.props.embedded) {
            return;
        }
        const ask = (event: BeforeUnloadEvent): void => event.preventDefault();
        window.addEventListener("beforeunload", ask);
        return () => window.removeEventListener("beforeunload", ask);
    }

    /** While changes are not saved, leaving saves them first; failing to, the user stays. */
    @effect guardChanges(): (() => void) | void {
        if (!this.isDirty || this.props.embedded) {
            return;
        }
        this.router.guard = () => this.save({ open: false });
        return () => {
            this.router.guard = null;
        };
    }

    /** Where the record stands among those of the list it was opened from, if it was. */
    @computed get pager(): { position: number; total: number; previous: number | null; next: number | null } | null {
        const memory = this.props.embedded || this.props.besideList ? undefined : listMemory(listKey(this.router.route));
        const at = memory?.ids.indexOf(this.props.resId ?? -1) ?? -1;
        if (memory === undefined || at < 0) {
            return null;
        }
        return {
            position: memory.offset + at + 1,
            total: memory.total,
            previous: memory.ids[at - 1] ?? null,
            next: memory.ids[at + 1] ?? null,
        };
    }

    /**
     * Once the page is scrolled past the leader, it keeps to one line, and grows back near the
     * top. The two points are apart, so that its change of height cannot flip it back — and it
     * only shrinks on a page long enough to stay scrolled past it once shorter by what it loses:
     * else the browser would scroll back up, the leader grow again, and so on without end.
     */
    @effect shrinkLeader(): (() => void) | void {
        if (!this.layout?.hasLeader || this.props.embedded) {
            return;
        }
        const follow = (): void => {
            const leader = this.element?.querySelector<HTMLElement>(".o_leader");
            const loses = this.leaderCompact ? 0 : (leader?.offsetHeight ?? 0) * LEADER_SHRINKS_BY;
            const scrollable = document.documentElement.scrollHeight - window.innerHeight;
            if (window.scrollY > LEADER_SHRINKS_PAST && scrollable - loses > LEADER_SHRINKS_PAST) {
                this.leaderCompact = true;
            } else if (window.scrollY < LEADER_GROWS_BEFORE) {
                this.leaderCompact = false;
            }
        };
        follow();
        window.addEventListener("scroll", follow, { passive: true });
        return () => window.removeEventListener("scroll", follow);
    }

    /** How many lines a one2many or many2many holds, as the user sees them. */
    lineCount(name: string): number {
        const value = this.current[name];
        return Array.isArray(value) ? value.length : 0;
    }

    /** The initials of a field's value, as its avatar shows them. */
    initials(name: string): string {
        return initialsOf(this.display(name));
    }

    avatarStyle(name: string): string {
        return avatarStyleOf(this.display(name));
    }

    /**
     * The names of the records its related links show, by field then id; read with the record.
     * A list computed from nothing it mirrors does not say what it holds: its link's action does.
     */
    @resource accessor relatedNames: Record<string, Record<number, string | null>> = load(
        () => {
            const asked: { name: string; model: string | undefined; action: string; ids: number[] }[] = [];
            if (loading(() => this.record) || loading(() => this.fields)) {
                return asked;
            }
            for (const { name, action } of this.layout?.relatedFields ?? []) {
                const ids = this.idsOf(name, this.record?.[name]);
                if (Array.isArray(ids) && ids.length > 0) {
                    asked.push({ name, model: this.fields?.[name]?.relation, action, ids: ids as number[] });
                }
            }
            return asked;
        },
        async (asked) => {
            const named = await Promise.all(
                asked.map(async ({ model, action, ids }) => {
                    const holding =
                        model ?? (await this.orm.call<{ model: string }>("action", "load", [], { xml_id: action })).model;
                    return this.orm.names(holding, ids);
                }),
            );
            return Object.fromEntries(asked.map(({ name }, at) => [name, Object.fromEntries(named[at])]));
        },
    );

    /** The records a one2many or many2many holds, with their names. */
    relatedRecords(name: string): { id: number; name: string }[] {
        const value = this.current[name];
        if (!Array.isArray(value)) {
            return [];
        }
        const names = loading(() => this.relatedNames) ? {} : (this.relatedNames?.[name] ?? {});
        return value.flatMap((item): { id: number; name: string }[] => {
            if (typeof item === "number") {
                return [{ id: item, name: names[item] ?? `#${item}` }];
            }
            if (Array.isArray(item) && typeof item[0] === "number") {
                return [{ id: item[0], name: (item[1] as string | null) ?? `#${item[0]}` }];
            }
            return [];
        });
    }

    /** Under a related link: the record's name, the first one's and how many more, or none. */
    relatedSummary(name: string): string {
        const records = this.relatedRecords(name);
        if (records.length === 0) {
            return "None yet";
        }
        return records.length === 1 ? records[0].name : `${records[0].name} and ${records.length - 1} more`;
    }

    /**
     * Follow a related link: its one record opens, several as a list; this one left in the trail.
     * A link saying `by` — the field of its records pointing back here — opens all of them as
     * this record's, where a new one is created as one of them: a project's board of tasks.
     */
    followLink(action: string, name: string, by: string | null = null): void {
        const records = this.relatedRecords(name);
        const id = this.props.resId;
        if (by !== null && id !== undefined) {
            void this.breadcrumb.openBy(action, by, id);
        } else if (records.length === 1) {
            void this.breadcrumb.open(action, records[0].id);
        } else if (records.length > 1) {
            void this.breadcrumb.openList(action, records.map((record) => record.id));
        }
    }

    /** Show another record of the list. */
    step(id: number | null): void {
        if (id !== null) {
            this.router.go({ ...this.router.route, id });
        }
    }

    /** A field's value as a condition reads it: records by their ids, not `[id, name]`. */
    conditionValue(name: string): unknown {
        return this.idsOf(name, this.current[name]) ?? null;
    }

    isBlank(name: string): boolean {
        const value = this.current[name];
        return value === null || value === undefined || value === "" || (Array.isArray(value) && value.length === 0);
    }

    /** The page a pages element shows: the one the user opened while shown, else the first shown. */
    shownPage(key: number, shown: boolean[]): number {
        const opened = this.openPages.get(key);
        if (opened !== undefined && shown[opened]) {
            return opened;
        }
        return shown.indexOf(true);
    }

    openPage(key: number, page: number): void {
        this.openPages.set(key, page);
    }

    /** What a widget calls with the value the user gave a field. */
    changer(name: string): (value: unknown) => void {
        return (value) => {
            this.changes[name] = value;
            this.scheduleOnchange();
        };
    }

    discard(): void {
        this.changes = {};
        this.forgetComputed();
        this.failure = null;
        this.tried = false;
    }

    /** Ask the server, once changing pauses, what the changes make of the fields computed from them. */
    private scheduleOnchange(): void {
        clearTimeout(this.onchangeTimer);
        this.onchangeTimer = setTimeout(() => void this.onchange(), ONCHANGE_AFTER);
    }

    /**
     * What the server computes from the record as the user changed it, shown until saved.
     *
     * Sends what saving would — every value for a new record, what changed for one that exists —
     * along with each one2many shown, so that its lines are computed with the record. An answer
     * overtaken by a later change is dropped. What could not be computed is marked beside its
     * field and told once; a call that fails altogether leaves the form as it was, and says so.
     */
    private async onchange(): Promise<void> {
        const fields = this.fields;
        if (fields === undefined) {
            return;
        }
        const asked = ++this.onchanges;
        const values = this.forServer(this.isNew ? { ...this.record, ...this.changes } : this.changes, true);
        delete values.id;
        for (const column of this.columns) {
            if (fields[column.name]?.relation_kind === "one2many" && !(column.name in values)) {
                values[column.name] = {};
            }
        }
        try {
            const answer = await this.orm.onchange(this.props.resModel, this.props.resId, values);
            if (asked !== this.onchanges) {
                return;
            }
            this.computedValues = answer.values;
            this.computeErrors = Object.fromEntries(answer.errors.map(({ field, message }) => [field, message]));
            const lineErrors: Record<string, Record<string, Record<string, string>>> = {};
            for (const [name, { errors }] of Object.entries(answer.lines)) {
                for (const { id, draft, field, message } of errors) {
                    const key = id === undefined ? `draft:${draft}` : `id:${id}`;
                    ((lineErrors[name] ??= {})[key] ??= {})[field] = message;
                    this.tell(`${fields[name]?.label ?? name}: ${field}`, message);
                }
            }
            this.lineErrors = lineErrors;
            for (const { field, message } of answer.errors) {
                this.tell(fields[field]?.label ?? field, message);
            }
            this.computedLines = Object.fromEntries(
                Object.entries(answer.lines).map(([name, { updated, created }]) => [
                    name,
                    Object.fromEntries([
                        ...updated.map(({ id, values: line }) => [`id:${id}`, line]),
                        ...created.map(({ draft, values: line }) => [`draft:${draft}`, line]),
                    ]),
                ]),
            );
        } catch (error) {
            if (asked === this.onchanges) {
                this.tell("The form", error instanceof Error ? error.message : String(error));
            }
        }
    }

    /** Warn that something could not be computed, once for each thing and reason. */
    private tell(what: string, why: string): void {
        const told = `${what}\n${why}`;
        if (this.told.has(told)) {
            return;
        }
        this.told.add(told);
        this.notifications.add("warning", `${what} could not be computed: ${why}`);
    }

    /** Forget what the server computed, and any answer still to come: the record is read again. */
    private forgetComputed(): void {
        clearTimeout(this.onchangeTimer);
        this.onchanges++;
        this.computedValues = {};
        this.computedLines = {};
        this.computeErrors = {};
        this.lineErrors = {};
        this.told.clear();
    }

    /**
     * Write what changed, or create the record, then show it as the server has it.
     *
     * The record created is opened unless `open` is false: when saving on the way somewhere else.
     * Notified while it saves, then once saved; why it was not is shown above the form too.
     * Returns whether it was saved — with nothing to save, it was.
     */
    async save(options: { open?: boolean } = {}): Promise<boolean> {
        if (!this.isDirty && !this.isNew) {
            return true;
        }
        this.tried = true;
        await nextTick();
        if (this.element?.querySelector(".o_form_missing")) {
            this.refuse("Some required fields are empty.");
            return false;
        }
        const notice = this.notifications.add("info", "Saving…", { sticky: true });
        this.saving = true;
        this.failure = null;
        try {
            const model = this.props.resModel;
            const id = this.props.resId;
            if (id === undefined) {
                const values = this.forServer({ ...this.record, ...this.changes });
                delete values.id;
                const [created] = await this.orm.create(model, values);
                this.orm.touch(model);
                this.changes = {};
                this.forgetComputed();
                this.tried = false;
                if (this.props.onCreated !== undefined) {
                    this.props.onCreated([created, this.nameOf(values)]);
                    return true;
                }
                // Saved: nothing is left to guard, though the guard goes only once effects run.
                this.router.guard = null;
                this.notifications.add("success", "Record created.");
                if (options.open !== false) {
                    void this.router.go({ ...this.router.route, view: "form", id: created });
                }
                return true;
            }
            await this.orm.write(model, [id], this.forServer(this.changes));
            this.record = await this.read(model, id, Object.keys(this.record));
            this.orm.touchRecords(model, [id]);
            this.changes = {};
            this.forgetComputed();
            this.tried = false;
            this.notifications.add("success", `${this.title} saved.`);
            return true;
        } catch (error) {
            this.refuse(error instanceof Error ? error.message : String(error));
            return false;
        } finally {
            this.saving = false;
            this.notifications.remove(notice);
        }
    }

    /** The name a record is created with: the value of the field naming the model's records. */
    private nameOf(values: Values): string | null {
        const field = Object.entries(this.fields ?? {}).find(([, described]) => described.name_field)?.[0];
        const name = field === undefined ? undefined : values[field];
        return typeof name === "string" && name !== "" ? name : null;
    }

    /**
     * Values as the server reads them: records by their ids, not `[id, name]`. With `drafts`, the
     * lines created say their draft number, as an onchange names them.
     */
    private forServer(values: Values, drafts = false): Values {
        return Object.fromEntries(
            Object.entries(values).map(([name, value]) => [
                name,
                this.fields?.[name]?.type === "refs" ? this.commandsOf(name, value, drafts) : this.idsOf(name, value),
            ]),
        );
    }

    /** A many2one's value as the id of its record; a one2many's or many2many's as their ids. */
    private idsOf(name: string, value: unknown): unknown {
        const type = this.fields?.[name]?.type;
        if (type === "ref" && Array.isArray(value)) {
            return value[0];
        }
        if (type === "refs" && Array.isArray(value)) {
            return value.flatMap((item: unknown) => {
                if (Array.isArray(item)) {
                    return [item[0]];
                }
                if (item !== null && typeof item === "object") {
                    const { id } = item as { id?: number };
                    return id === undefined ? [] : [id];
                }
                return [item];
            });
        }
        return value;
    }

    /**
     * What changed in a one2many or a many2many, as the commands the server carries out on what
     * it holds: records let go, changed, created, and added.
     */
    private commandsOf(name: string, value: unknown, drafts = false): Values {
        const held = new Set(this.idsOf(name, this.record?.[name] ?? []) as number[]);
        const items = Array.isArray(value) ? value : [];
        const update: Values[] = [];
        const create: Values[] = [];
        const kept = new Set<number>();
        for (const item of items) {
            if (item !== null && typeof item === "object" && !Array.isArray(item)) {
                const { id, draft, values } = item as { id?: number; draft?: number; values: Values };
                const sent = Object.fromEntries(Object.entries(values).map(([field, inner]) => [field, asSent(inner)]));
                if (id === undefined) {
                    create.push(drafts && draft !== undefined ? { draft, ...sent } : sent);
                } else {
                    kept.add(id);
                    update.push({ id, ...sent });
                }
            } else {
                kept.add(Array.isArray(item) ? (item[0] as number) : (item as number));
            }
        }
        const commands: Values = {
            unlink: [...held].filter((id) => !kept.has(id)),
            update,
            create,
            link: [...kept].filter((id) => !held.has(id)),
        };
        return Object.fromEntries(Object.entries(commands).filter(([, list]) => (list as unknown[]).length > 0));
    }

    private refuse(reason: string): void {
        this.failure = reason;
        this.notifications.add("danger", `Not saved: ${reason}`);
    }

    /**
     * Run a button: a method of the record, saved first, then read again; or another action.
     *
     * A record not created yet is created first, and shown; its button is pressed from there.
     */
    async press(button: FormButton, at: number): Promise<void> {
        if (button.type === "cancel") {
            this.props.onClose?.();
            return;
        }
        if (button.type === "action") {
            this.router.go({ action: button.name, view: null, id: null });
            return;
        }
        if (this.busy) {
            return;
        }
        this.pressing = at;
        try {
            await (this.props.dialog ? this.runInDialog(button) : this.run(button));
        } finally {
            this.pressing = null;
        }
    }

    /**
     * Run a button of a wizard: create the record with what the user filled in, run the method on
     * it, and close the dialog. Should the method fail, the record created goes, and why stays shown.
     */
    private async runInDialog(button: FormButton): Promise<void> {
        this.tried = true;
        await nextTick();
        if (this.element?.querySelector(".o_form_missing")) {
            this.refuse("Some required fields are empty.");
            return;
        }
        this.failure = null;
        const model = this.props.resModel;
        let created: number | null = null;
        try {
            const values = this.forServer({ ...this.record, ...this.changes });
            delete values.id;
            [created] = await this.orm.create(model, values);
            const answer = await this.orm.call(model, button.name, [created]);
            this.props.onDone?.(answer);
            this.props.onClose?.();
        } catch (error) {
            if (created !== null) {
                await this.orm.delete(model, [created]).catch(() => 0);
            }
            this.refuse(error instanceof Error ? error.message : String(error));
        }
    }

    private async run(button: FormButton): Promise<void> {
        const id = this.props.resId;
        if (!(await this.save()) || id === undefined) {
            return;
        }
        try {
            const answer = await this.orm.call(this.props.resModel, button.name, [id]);
            if (asksReload(answer)) {
                window.location.reload();
                return;
            }
            const opened = opensRecord(answer);
            if (opened !== null) {
                await (opened.ids === null
                    ? this.breadcrumb.open(opened.action, opened.id)
                    : this.breadcrumb.openList(opened.action, opened.ids));
                return;
            }
            this.record = await this.read(this.props.resModel, id, Object.keys(this.record));
            this.orm.touchRecords(this.props.resModel, [id]);
        } catch (error) {
            this.failure = error instanceof Error ? error.message : String(error);
        }
    }
}

/**
 * A value of a line, as the server reads it, from its shape alone — the form does not know the
 * line's fields: a record as `[id, name]` by its id, records as such by their ids.
 */
function asSent(value: unknown): unknown {
    const isNamed = (item: unknown): boolean =>
        Array.isArray(item) && item.length === 2 && typeof item[0] === "number" && (typeof item[1] === "string" || item[1] === null);
    if (isNamed(value)) {
        return (value as [number, unknown])[0];
    }
    if (Array.isArray(value) && value.length > 0 && value.every(isNamed)) {
        return value.map((item) => (item as [number, unknown])[0]);
    }
    return value;
}

viewKinds.add("form", FormView);
