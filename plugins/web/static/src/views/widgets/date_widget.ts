import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** A day, written `YYYY-MM-DD` by the server and read in the browser's time zone, not UTC's. */
export class DateWidget extends Widget {
    override props = props({ ...widgetProps });

    override get text(): string {
        if (this.isEmpty) {
            return "";
        }
        const [year, month, day] = String(this.value).split("-").map(Number);
        return new Date(year, month - 1, day).toLocaleDateString();
    }

    override get inputType(): string {
        return "date";
    }

    override parse(text: string): unknown {
        return text === "" ? null : text;
    }
}

widgets.add("date", DateWidget);
