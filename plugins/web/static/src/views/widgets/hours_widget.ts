import { props, state } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * A number of hours as hours and minutes: `1:30` for 1.5. Typed as `1:30`, or as hours — `1.5`,
 * `1,5` — the value changing as soon as what is typed reads as one; what is typed stays as typed
 * until the user leaves the field, which then shows the value as hours and minutes again.
 */
export class HoursWidget extends Widget {
    static override template = "web.HoursWidget";

    override props = props({ ...widgetProps });

    override get text(): string {
        return this.isEmpty ? "" : hoursText(Number(this.value));
    }

    /** What the user is typing, until they leave the field. */
    @state accessor typing: string | null = null;

    override get inputValue(): string {
        return this.typing ?? this.text;
    }

    override parse(text: string): unknown {
        const trimmed = text.trim();
        if (trimmed === "") {
            return null;
        }
        const hours = hoursOf(trimmed);
        return hours === null ? undefined : hours.toFixed(2);
    }

    /** Change the value as soon as what is typed reads as hours. */
    type(text: string): void {
        this.typing = text;
        const parsed = this.parse(text);
        if (parsed !== undefined) {
            this.props.onChange?.(parsed);
        }
    }

    /** Show the value again as hours and minutes. */
    leave(): void {
        this.typing = null;
    }
}

/** `1:30` for 1.5, `-0:15` for -0.25: the minutes rounded. */
export function hoursText(hours: number): string {
    if (!Number.isFinite(hours)) {
        return "";
    }
    const minutes = Math.round(Math.abs(hours) * 60);
    const sign = hours < 0 && minutes > 0 ? "-" : "";
    return `${sign}${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}`;
}

/** The hours `1:30`, `1:30:00`, `1.5` or `1,5` say; `null` for anything else. */
export function hoursOf(text: string): number | null {
    const negative = text.startsWith("-");
    const unsigned = negative ? text.slice(1).trim() : text;
    let hours: number;
    if (unsigned.includes(":")) {
        const parts = unsigned.split(":").map((part) => part.trim());
        if (parts.length > 3 || parts.some((part) => !/^\d+$/.test(part))) {
            return null;
        }
        const [whole, minutes, seconds = 0] = parts.map(Number);
        if (minutes >= 60 || seconds >= 60) {
            return null;
        }
        hours = whole + minutes / 60 + seconds / 3600;
    } else {
        if (!/^\d*[.,]?\d*$/.test(unsigned) || !/\d/.test(unsigned)) {
            return null;
        }
        hours = Number(unsigned.replace(",", "."));
    }
    return negative ? -hours : hours;
}

widgets.add("hours", HoursWidget);
