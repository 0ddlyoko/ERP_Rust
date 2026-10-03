import { inject, props } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import { actionFor, Menus } from "@web/core/menus";
import { type Choice, RecordSearch } from "./record_search";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * The record a many2one points to: its name when the view read it with names, as `[id, name]`;
 * its id otherwise, or when the user may not read it.
 *
 * Where a view edits it, it is chosen by searching ([`RecordSearch`](./record_search.ts));
 * emptying it clears it. Its record opens in a form when a menu leads to its model, the record
 * left in the breadcrumb.
 */
export class Many2OneWidget extends Widget {
    static override template = "web.Many2OneWidget";
    static components = { RecordSearch };

    override props = props({ ...widgetProps });

    @inject(Breadcrumb) breadcrumb!: Breadcrumb;
    @inject(Menus) menus!: Menus;

    override get text(): string {
        if (this.isEmpty) {
            return "";
        }
        if (Array.isArray(this.value)) {
            const [id, name] = this.value as [number, string | null];
            return name ?? `#${id}`;
        }
        return `#${this.value}`;
    }

    get id(): number | null {
        if (this.isEmpty) {
            return null;
        }
        return Array.isArray(this.value) ? (this.value[0] as number) : (this.value as number);
    }

    /** The action a menu opens on the model pointed to, to open its record with. */
    get openAction(): string | null {
        return actionFor(this.menus.tree ?? [], this.props.field.relation);
    }

    readonly pick = (choice: Choice): void => {
        this.props.onChange?.(choice);
    };

    readonly clear = (): void => {
        if (!this.isEmpty) {
            this.props.onChange?.(null);
        }
    };

    async openRecord(): Promise<void> {
        const action = this.openAction;
        const id = this.id;
        if (action !== null && id !== null) {
            await this.breadcrumb.open(action, id);
        }
    }
}

widgets.add("many2one", Many2OneWidget);
