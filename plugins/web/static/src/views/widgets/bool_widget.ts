import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** A check box, which only an editing view lets the user tick. */
export class BoolWidget extends Widget {
    static override template = "web.BoolWidget";

    override props = props({ ...widgetProps });

    toggle(checked: boolean): void {
        this.props.onChange?.(checked);
    }
}

widgets.add("bool", BoolWidget);
