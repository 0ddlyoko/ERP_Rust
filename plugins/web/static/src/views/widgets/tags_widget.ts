import { inject, props, state } from "trame";
import { Models } from "@web/core/models";
import { Orm, type Values } from "@web/core/orm";
import { FormDialog } from "@web/views/form/form_dialog";
import { createByName, nameDefaults } from "./record_creation";
import { RecordSearch } from "./record_search";
import { widgetProps, widgets } from "./widget";
import { X2ManyWidget } from "./x2many_widget";

/**
 * The records of a one2many or a many2many as tags, one per record, by name.
 *
 * Where a view edits it, a tag is removed with its cross, and one added by searching. A name
 * matching nothing can become a new record, unless the field's element says `no_create="1"`: for
 * a many2many, created at once or through a form in a dialog; for a one2many, a line created with
 * the record holding it.
 */
export class TagsWidget extends X2ManyWidget {
    static override template = "web.TagsWidget";
    static components = { RecordSearch, FormDialog };

    override props = props({ ...widgetProps });

    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;

    /** What the dialog creating a record starts with, while it is open. */
    @state accessor creating: Values | null = null;

    override get text(): string {
        return this.entries.map((entry) => this.nameOf(entry)).join(", ");
    }

    get relation(): string {
        return this.props.field.relation ?? "";
    }

    get canCreate(): boolean {
        return this.props.attrs.no_create !== "1";
    }

    get isOne2Many(): boolean {
        return this.props.field.relation_kind === "one2many";
    }

    /** Backspace in the empty search removes the last tag, as in a mail's recipients. */
    readonly removeLast = (): void => {
        const last = this.entries.at(-1);
        if (last !== undefined) {
            this.remove(last.key);
        }
    };

    readonly create = async (name: string): Promise<void> => {
        if (this.isOne2Many) {
            this.addDraft(await nameDefaults(this.models, this.relation, name));
            return;
        }
        const created = await createByName(this.orm, this.relation, name);
        if (created === null) {
            await this.createEdit(name);
        } else {
            this.add(created);
        }
    };

    readonly createEdit = async (name: string): Promise<void> => {
        this.creating = await nameDefaults(this.models, this.relation, name);
    };

    readonly created = ([id, name]: [number, string | null]): void => {
        this.add([id, name ?? `#${id}`]);
    };

    readonly closeDialog = (): void => {
        this.creating = null;
    };
}

widgets.add("tags", TagsWidget);
