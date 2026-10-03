import { props } from "trame";
import { RecordSearch } from "./record_search";
import { widgetProps, widgets } from "./widget";
import { X2ManyWidget } from "./x2many_widget";

/**
 * The records of a one2many or a many2many as tags, one per record, by name.
 *
 * Where a view edits it, a tag is removed with its cross, and one added by searching.
 */
export class TagsWidget extends X2ManyWidget {
    static override template = "web.TagsWidget";
    static components = { RecordSearch };

    override props = props({ ...widgetProps });

    override get text(): string {
        return this.entries.map((entry) => this.nameOf(entry)).join(", ");
    }
}

widgets.add("tags", TagsWidget);
