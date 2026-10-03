import { inject } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import { actionFor, Menus } from "@web/core/menus";
import type { Choice } from "./record_search";
import { Widget } from "./widget";

/** A record a one2many or a many2many holds: its id and its name, `null` when out of reach. */
export type Linked = [number, string | null];

/**
 * What the widgets of a one2many or a many2many share, whichever kind they show: the records
 * held, read with names as `[[id, name], ...]` or as ids alone, and their changes.
 *
 * The field changes as a whole: the records it now holds, each with its name.
 */
export abstract class X2ManyWidget extends Widget {
    @inject(Breadcrumb) breadcrumb!: Breadcrumb;
    @inject(Menus) menus!: Menus;

    get linked(): Linked[] {
        const value = Array.isArray(this.value) ? (this.value as unknown[]) : [];
        return value.map((item) => (Array.isArray(item) ? (item as Linked) : [item as number, null]));
    }

    get ids(): number[] {
        return this.linked.map(([id]) => id);
    }

    nameOf([id, name]: Linked): string {
        return name ?? `#${id}`;
    }

    readonly add = (choice: Choice): void => {
        this.props.onChange?.([...this.linked, choice]);
    };

    remove(id: number): void {
        this.props.onChange?.(this.linked.filter(([linked]) => linked !== id));
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
