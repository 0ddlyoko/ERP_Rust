import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

export class IntegerWidget extends Widget {
    override props = props({ ...widgetProps });
}

widgets.add("integer", IntegerWidget);
