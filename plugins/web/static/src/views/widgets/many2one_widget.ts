import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * The record a many2one points to: its name when the view read it with names, as `[id, name]`;
 * its id otherwise, or when the user may not read it.
 */
export class Many2OneWidget extends Widget {
    override props = props({ ...widgetProps });

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
}

widgets.add("many2one", Many2OneWidget);
