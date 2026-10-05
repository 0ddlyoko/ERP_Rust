import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** Text over several lines, such as notes: kept as typed, line breaks included. */
export class TextWidget extends Widget {
    static override template = "web.TextWidget";

    override props = props({ ...widgetProps });
}

widgets.add("text", TextWidget);
