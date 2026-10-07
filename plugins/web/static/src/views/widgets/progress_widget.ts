import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * A percentage as a bar: how much of a budget is spent. It turns orange from `warn` percent —
 * 80 unless the view says — and red past 100, the bar full.
 */
export class ProgressWidget extends Widget {
    static override template = "web.ProgressWidget";

    override props = props({ ...widgetProps });

    get percent(): number {
        const value = Number(this.value ?? 0);
        return Number.isFinite(value) ? Math.round(value) : 0;
    }

    get level(): "fine" | "warn" | "over" {
        const warn = Number((this.props.attrs as Record<string, string>).warn ?? 80);
        return this.percent > 100 ? "over" : this.percent >= warn ? "warn" : "fine";
    }

    get width(): string {
        return `width: ${Math.min(100, Math.max(0, this.percent))}%`;
    }
}

widgets.add("progress", ProgressWidget);
