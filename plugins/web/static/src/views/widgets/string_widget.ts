import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** Text, as it is. */
export class StringWidget extends Widget {
    override props = props({ ...widgetProps });
}

widgets.add("string", StringWidget);
