import { inject } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import { actionFor, Menus } from "@web/core/menus";
import type { Values } from "@web/core/orm";
import type { Choice } from "./record_search";
import { Widget } from "./widget";

/** A record a one2many or a many2many holds: its id and its name, `null` when out of reach. */
export type Linked = [number, string | null];

/** A record held, changed here: an existing one by its `id`, a new one by a `draft` number. */
export interface Edited {
    id?: number;
    draft?: number;
    name?: string | null;
    values: Values;
}

/** A record held, as a widget shows and changes it. */
export interface Entry {
    /** Stable while the record is held: `id:3`, or `draft:1` for one not created yet. */
    key: string;
    id: number | null;
    name: string | null;
    /** What was changed here, to save with the record holding it; null when nothing was. */
    changes: Values | null;
    draft: number | null;
}

/**
 * What the widgets of a one2many or a many2many share, whichever kind they show: the records
 * held, and their changes until the record holding them is saved.
 *
 * The value read is `[[id, name], ...]`, or ids alone. Changed here, it also holds records
 * changed (`{id, values}`) and records to create (`{draft, values}`), which the form sends as
 * commands.
 */
export abstract class X2ManyWidget extends Widget {
    @inject(Breadcrumb) breadcrumb!: Breadcrumb;
    @inject(Menus) menus!: Menus;

    get entries(): Entry[] {
        const value = Array.isArray(this.value) ? (this.value as unknown[]) : [];
        return value.map((item): Entry => {
            if (Array.isArray(item)) {
                const [id, name] = item as Linked;
                return { key: `id:${id}`, id, name, changes: null, draft: null };
            }
            if (typeof item === "number") {
                return { key: `id:${item}`, id: item, name: null, changes: null, draft: null };
            }
            const edited = item as Edited;
            if (edited.id !== undefined) {
                return { key: `id:${edited.id}`, id: edited.id, name: edited.name ?? null, changes: edited.values, draft: null };
            }
            const draft = edited.draft ?? 0;
            return { key: `draft:${draft}`, id: null, name: null, changes: edited.values, draft };
        });
    }

    /** The records held as `[id, name]`, those not created yet left out. */
    get linked(): Linked[] {
        return this.entries.filter((entry) => entry.id !== null).map((entry) => [entry.id as number, entry.name]);
    }

    get ids(): number[] {
        return this.linked.map(([id]) => id);
    }

    nameOf(entry: Entry): string {
        const named = entry.changes?.name;
        if (typeof named === "string" && named !== "") {
            return named;
        }
        return entry.name ?? (entry.id === null ? "New" : `#${entry.id}`);
    }

    /** The value holding these entries, as the form keeps it. */
    protected override valueOf(entries: Entry[]): unknown[] {
        return entries.map((entry) => {
            if (entry.id === null) {
                return { draft: entry.draft, values: entry.changes ?? {} };
            }
            if (entry.changes === null) {
                return [entry.id, entry.name];
            }
            return { id: entry.id, name: entry.name, values: entry.changes };
        });
    }

    protected update(entries: Entry[]): void {
        this.props.onChange?.(this.valueOf(entries));
    }

    readonly add = (choice: Choice): void => {
        const [id, name] = choice;
        this.update([...this.entries, { key: `id:${id}`, id, name, changes: null, draft: null }]);
    };

    /** Hold a record not created yet, with these values; its key, to edit it by. */
    addDraft(values: Values = {}): string {
        const draft = Math.max(0, ...this.entries.map((entry) => entry.draft ?? 0)) + 1;
        const key = `draft:${draft}`;
        this.update([...this.entries, { key, id: null, name: null, changes: values, draft }]);
        return key;
    }

    remove(key: string): void {
        this.update(this.entries.filter((entry) => entry.key !== key));
    }

    /** Change one field of a record held, kept until the record holding it is saved. */
    change(key: string, name: string, value: unknown): void {
        this.update(
            this.entries.map((entry) =>
                entry.key === key ? { ...entry, changes: { ...(entry.changes ?? {}), [name]: value } } : entry,
            ),
        );
    }

    /** The action a menu opens on the model held, to open its records with. */
    get openAction(): string | null {
        return actionFor(this.menus.tree ?? [], this.props.field.relation);
    }

    /** Open one of the records, the record left in the breadcrumb. */
    async open(id: number): Promise<void> {
        const action = this.openAction;
        if (action !== null) {
            await this.breadcrumb.open(action, id);
        }
    }
}
