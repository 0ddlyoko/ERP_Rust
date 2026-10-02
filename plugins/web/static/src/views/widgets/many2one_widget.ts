import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** The record a many2one points to: its id, until records have names. */
export class Many2OneWidget extends Widget {
    override props = props({ ...widgetProps });

    override get text(): string {
        return this.isEmpty ? "" : `#${this.value}`;
    }
}

widgets.add("many2one", Many2OneWidget);
