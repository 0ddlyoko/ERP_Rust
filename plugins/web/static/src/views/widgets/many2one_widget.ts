import { inject, props, state } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import { actionFor, Menus } from "@web/core/menus";
import { Models } from "@web/core/models";
import { Orm, type Values } from "@web/core/orm";
import { FormDialog } from "@web/views/form/form_dialog";
import { createByName, nameDefaults } from "./record_creation";
import { type Choice, RecordSearch } from "./record_search";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * The record a many2one points to: its name when the view read it with names, as `[id, name]`; its
 * id otherwise, or when the user may not read it.
 *
 * Where a view edits it, it is chosen by searching ([`RecordSearch`](./record_search.ts)); emptying
 * it clears it. Its record opens in a form when a menu leads to its model, the record left in the
 * breadcrumb — from a form, a click on it opens it too when shown only. A name matching nothing can
 * become a new record — at once, or through a form in a dialog — unless the field's element says
 * `no_create="1"`.
 */
export class Many2OneWidget extends Widget {
    static override template = "web.Many2OneWidget";
    static components = { RecordSearch, FormDialog };

    override props = props({ ...widgetProps });

    @inject(Breadcrumb) breadcrumb!: Breadcrumb;
    @inject(Menus) menus!: Menus;
    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;

    /** What the dialog creating a record starts with, while it is open. */
    @state accessor creating: Values | null = null;

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

    get canOpen(): boolean {
        return this.openAction !== null && this.id !== null;
    }

    readonly pick = (choice: Choice): void => {
        this.props.onChange?.(choice);
    };

    get canCreate(): boolean {
        return this.props.attrs.no_create !== "1";
    }

    get relation(): string {
        return this.props.field.relation ?? "";
    }

    /** A record created from the name typed; through the form when it needs more than a name. */
    readonly create = async (name: string): Promise<void> => {
        const created = await createByName(this.orm, this.relation, name);
        if (created === null) {
            await this.createEdit(name);
        } else {
            this.pick(created);
        }
    };

    readonly createEdit = async (name: string): Promise<void> => {
        this.creating = await nameDefaults(this.models, this.relation, name);
    };

    readonly created = ([id, name]: [number, string | null]): void => {
        this.props.onChange?.([id, name]);
    };

    readonly closeDialog = (): void => {
        this.creating = null;
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
