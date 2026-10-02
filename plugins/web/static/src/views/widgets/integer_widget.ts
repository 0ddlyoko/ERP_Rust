import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

export class IntegerWidget extends Widget {
    override props = props({ ...widgetProps });

    override get inputType(): string {
        return "number";
    }

    override parse(text: string): unknown {
        return text.trim() === "" ? null : Math.trunc(Number(text));
    }
}

widgets.add("integer", IntegerWidget);
