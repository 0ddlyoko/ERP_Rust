import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** A number in the browser's notation: `1 234,5` or `1,234.5`. */
export class DecimalWidget extends Widget {
    override props = props({ ...widgetProps });

    override get text(): string {
        return this.isEmpty ? "" : Number(this.value).toLocaleString();
    }
}

widgets.add("decimal", DecimalWidget);
