import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

export class DateTimeWidget extends Widget {
    override props = props({ ...widgetProps });

    override get text(): string {
        return this.isEmpty ? "" : new Date(String(this.value)).toLocaleString();
    }

    override get inputType(): string {
        return "datetime-local";
    }

    /** The moment in the browser's time zone, as a `datetime-local` input holds it. */
    override get inputValue(): string {
        if (this.isEmpty) {
            return "";
        }
        const moment = new Date(String(this.value));
        const local = new Date(moment.getTime() - moment.getTimezoneOffset() * 60000);
        return local.toISOString().slice(0, 16);
    }

    /** Back to UTC, as the server keeps a moment. */
    override parse(text: string): unknown {
        return text === "" ? null : new Date(text).toISOString();
    }
}

widgets.add("datetime", DateTimeWidget);
