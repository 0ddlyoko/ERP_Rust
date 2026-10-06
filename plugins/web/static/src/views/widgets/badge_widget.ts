import { props } from "trame";
import { decorationOf } from "@web/core/expression";
import { SelectionWidget } from "./selection_widget";
import { widgetProps, widgets } from "./widget";

/**
 * A value as a coloured pill: a state, a status. Its colour is the first `decoration-success`,
 * `-info`, `-warning`, `-danger` or `-muted` whose expression holds for the record; grey
 * otherwise. Chosen from a list like any selection, where a view edits it.
 */
export class BadgeWidget extends SelectionWidget {
    static override template = "web.BadgeWidget";

    override props = props({ ...widgetProps });

    get decoration(): string {
        return decorationOf(this.props.attrs as Record<string, string>, this.props.record) ?? "default";
    }
}

widgets.add("badge", BadgeWidget);
