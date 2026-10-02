import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

export class DateTimeWidget extends Widget {
    override props = props({ ...widgetProps });

    override get text(): string {
        return this.isEmpty ? "" : new Date(String(this.value)).toLocaleString();
    }
}

widgets.add("datetime", DateTimeWidget);
