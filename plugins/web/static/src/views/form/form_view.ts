import { type ComponentClass, computed, effect, inject, load, nextTick, props, resource, state } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import { listMemory } from "@web/core/list_memory";
import { Notifications } from "@web/core/notifications";
import type { Fields } from "@web/core/models";
import type { Values } from "@web/core/orm";
import { Router } from "@web/core/router";
import { View, viewKinds, viewProps } from "@web/views/view";
import { bodyFor } from "./form_body";
import { type CompiledForm, compileForm, type FormButton } from "./form_compiler";

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

    override props = props({ ...viewProps });

    @inject(Router) router!: Router;
    @inject(Breadcrumb) breadcrumb!: Breadcrumb;
    @inject(Notifications) notifications!: Notifications;

    override get kind(): string {
        return "form";
    }

    @state accessor changes: Values = {};
    @state accessor saving = false;
    @state accessor failure: string | null = null;
    @state accessor openPages = new Map<number, number>();
    /** Whether a save was tried: required fields left empty are shown from then on. */
    @state accessor tried = false;

    /** The form's body, set by its template. */
    element: HTMLElement | null = null;

    /** The record as read, or its fields' defaults for one not created yet. */
    @resource accessor record: Values = load(
        () => ({
            model: this.props.resModel,
            id: this.props.resId,
            names: this.readNames,
            fields: this.fields,
        }),
        async ({ model, id, names, fields }) => (id === undefined ? defaultsOf(fields, names) : this.read(model, id, names)),
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

    /** The fields read with the record: those shown, and those a condition reads. */
    @computed get readNames(): string[] {
        const fields = this.fields ?? {};
        const conditions = (this.layout?.conditionNames ?? []).filter((name) => name in fields);
        return [...new Set([...this.columns.map((column) => column.name), ...conditions])];
    }

    /** The record as the user sees it: what was read, with what they changed over it. */
    @computed get current(): Values {
        return { ...this.record, ...this.changes };
    }

    get isNew(): boolean {
        return this.props.resId === undefined;
    }

    get isDirty(): boolean {
        return Object.keys(this.changes).length > 0;
    }

    /** Something to save, and no save under way. */
    get canSave(): boolean {
        return !this.saving && (this.isDirty || this.isNew);
    }

    /** A label as written, each `{{ field }}` replaced by the field's value; `\{{` is a brace. */
    label(text: string): string {
        return text.replace(/(\\)?\{\{\s*(\w+)\s*\}\}/g, (written, escaped: string | undefined, name: string) =>
            escaped ? written.slice(1) : this.display(name),
        );
    }

    /** A field's value as a label reads it: a record's name, how many records, or as it is. */
    display(name: string): string {
        const value = this.current[name];
        if (value === null || value === undefined || value === false) {
            return "";
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

    /** The record's name, as the breadcrumb shows it. */
    get title(): string {
        if (this.isNew) {
            return "New";
        }
        return this.fields?.name !== undefined && this.display("name") ? this.display("name") : `#${this.props.resId}`;
    }

    @effect nameInBreadcrumb(): () => void {
        this.breadcrumb.record = this.title;
        return () => {
            this.breadcrumb.record = null;
        };
    }

    /** While changes are not saved, leaving saves them first; failing to, the user stays. */
    @effect guardChanges(): (() => void) | void {
        if (!this.isDirty) {
            return;
        }
        this.router.guard = () => this.save({ open: false });
        return () => {
            this.router.guard = null;
        };
    }

    /** Where the record stands among those of the list it was opened from, if it was. */
    @computed get pager(): { position: number; total: number; previous: number | null; next: number | null } | null {
        const memory = listMemory(this.router.route.action);
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

    /** Show another record of the list. */
    step(id: number | null): void {
        if (id !== null) {
            this.router.go({ ...this.router.route, id });
        }
    }

    /** A field's value as a condition reads it: a many2one as the id of its record. */
    conditionValue(name: string): unknown {
        const value = this.current[name];
        if (this.fields?.[name]?.type === "ref" && Array.isArray(value)) {
            return value[0];
        }
        return value ?? null;
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
        };
    }

    discard(): void {
        this.changes = {};
        this.failure = null;
        this.tried = false;
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
                const values = { ...this.record, ...this.changes };
                delete values.id;
                const [created] = await this.orm.create(model, values);
                this.changes = {};
                this.tried = false;
                // Saved: nothing is left to guard, though the guard goes only once effects run.
                this.router.guard = null;
                this.notifications.add("success", "Record created.");
                if (options.open !== false) {
                    void this.router.go({ ...this.router.route, view: "form", id: created });
                }
                return true;
            }
            await this.orm.write(model, [id], this.changes);
            this.record = await this.read(model, id, Object.keys(this.record));
            this.changes = {};
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

    private refuse(reason: string): void {
        this.failure = reason;
        this.notifications.add("danger", `Not saved: ${reason}`);
    }

    /**
     * Run a button: a method of the record, saved first, then read again; or another action.
     *
     * A record not created yet is created first, and shown; its button is pressed from there.
     */
    async press(button: FormButton): Promise<void> {
        if (button.type === "action") {
            this.router.go({ action: button.name, view: null, id: null });
            return;
        }
        const id = this.props.resId;
        if (!(await this.save()) || id === undefined) {
            return;
        }
        try {
            await this.orm.call(this.props.resModel, button.name, [id]);
            this.record = await this.read(this.props.resModel, id, Object.keys(this.record));
        } catch (error) {
            this.failure = error instanceof Error ? error.message : String(error);
        }
    }
}

/** A new record's values: each field's default, for the fields the form shows. */
function defaultsOf(fields: Fields, names: string[]): Values {
    return Object.fromEntries(
        names.filter((name) => name !== "id" && fields[name]?.default !== undefined).map((name) => [name, fields[name].default]),
    );
}

viewKinds.add("form", FormView);
