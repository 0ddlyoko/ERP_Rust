import { computed, inject, load, props, resource, state } from "trame";
import type { Fields } from "@web/core/models";
import type { Values } from "@web/core/orm";
import { Router } from "@web/core/router";
import { type Column, View, viewKinds, viewProps } from "@web/views/view";

/** A piece of a heading: text as written, or a field's value. */
export type HeadingPart = { text: string } | { column: Column };

export interface FormPage {
    name: string;
    title: string;
    children: FormNode[];
}

export interface FormButton {
    name: string;
    type: "method" | "action";
    title: string;
}

/** An element of a form's XML, as it is laid out. */
export type FormNode =
    | { type: "block"; key: string; title: string | null; children: FormNode[] }
    | { type: "field"; key: string; column: Column }
    | { type: "heading"; key: string; level: number; parts: HeadingPart[] }
    | { type: "pages"; key: string; pages: FormPage[] };

/**
 * One record, its fields laid out as its view's XML says, edited in place.
 *
 * What the user changes is kept apart from what was read: saving writes only that, discarding
 * forgets it. A record not created yet starts from its fields' defaults, and saving creates it.
 * Labels may show a field's value — `string="Groups of {{ name }}"` — and follow it as it is
 * edited.
 */
export class FormView extends View {
    static template = "web.FormView";

    override props = props({ ...viewProps });

    @inject(Router) router!: Router;

    override get kind(): string {
        return "form";
    }

    @state accessor changes: Values = {};
    @state accessor saving = false;
    @state accessor failure: string | null = null;
    @state accessor openPages = new Map<string, string>();

    /** The record as read, or its fields' defaults for one not created yet. */
    @resource accessor record: Values = load(
        () => ({
            model: this.props.resModel,
            id: this.props.resId,
            names: [...new Set(this.columns.map((column) => column.name))],
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

    /** The form's elements, laid out; buttons aside, shown above it. */
    @computed get nodes(): FormNode[] {
        const root = this.archRoot;
        const fields = this.fields;
        if (root === undefined || fields === undefined) {
            return [];
        }
        return this.nodesOf(root, fields, "");
    }

    @computed get buttons(): FormButton[] {
        const root = this.archRoot;
        if (root === undefined) {
            return [];
        }
        return Array.from(root.children)
            .filter((element) => element.tagName === "buttons")
            .flatMap((element) => Array.from(element.children))
            .map((button) => ({
                name: button.getAttribute("name") ?? "",
                type: button.getAttribute("type") === "action" ? "action" : "method",
                title: button.getAttribute("string") ?? button.getAttribute("name") ?? "",
            }));
    }

    private nodesOf(parent: Element, fields: Fields, path: string): FormNode[] {
        return Array.from(parent.children).flatMap((element, index): FormNode[] => {
            const key = `${path}${index}`;
            const tag = element.tagName;
            if (tag === "block") {
                const title = element.getAttribute("string");
                return [{ type: "block", key, title, children: this.nodesOf(element, fields, `${key}.`) }];
            }
            if (tag === "field") {
                return [{ type: "field", key, column: this.columnOf(element, fields) }];
            }
            if (/^h[1-6]$/.test(tag)) {
                return [{ type: "heading", key, level: Number(tag.slice(1)), parts: this.partsOf(element, fields) }];
            }
            if (tag === "pages") {
                const pages = Array.from(element.children).map((page, at) => ({
                    name: page.getAttribute("name") ?? String(at),
                    title: page.getAttribute("string") ?? page.getAttribute("name") ?? "",
                    children: this.nodesOf(page, fields, `${key}.${at}.`),
                }));
                return [{ type: "pages", key, pages }];
            }
            return [];
        });
    }

    private partsOf(heading: Element, fields: Fields): HeadingPart[] {
        return Array.from(heading.childNodes).flatMap((node): HeadingPart[] => {
            if (node.nodeType === Node.TEXT_NODE) {
                const text = node.textContent ?? "";
                return text.trim() ? [{ text }] : [];
            }
            if (node instanceof Element && node.tagName === "field") {
                return [{ column: this.columnOf(node, fields) }];
            }
            return [];
        });
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

    /** What a widget calls with the value the user gave a field. */
    changer(name: string): (value: unknown) => void {
        return (value) => {
            this.changes[name] = value;
        };
    }

    isOpen(pages: { key: string; pages: FormPage[] }, page: FormPage): boolean {
        return (this.openPages.get(pages.key) ?? pages.pages[0]?.name) === page.name;
    }

    openPage(pages: { key: string }, page: FormPage): void {
        this.openPages.set(pages.key, page.name);
    }

    discard(): void {
        this.changes = {};
        this.failure = null;
    }

    /**
     * Write what changed, or create the record, then show it as the server has it.
     *
     * Returns whether it was saved; why not is shown above the form.
     */
    async save(): Promise<boolean> {
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
                this.router.go({ ...this.router.route, view: "form", id: created });
                return true;
            }
            if (this.isDirty) {
                await this.orm.write(model, [id], this.changes);
                this.record = await this.read(model, id, Object.keys(this.record));
                this.changes = {};
            }
            return true;
        } catch (error) {
            this.failure = error instanceof Error ? error.message : String(error);
            return false;
        } finally {
            this.saving = false;
        }
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
