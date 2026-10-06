import { props } from "trame";
import { type Choice, SelectionWidget } from "./selection_widget";
import { widgetProps, widgets } from "./widget";

/**
 * A selection as the steps it goes through, in its values' order: those passed, the one it is
 * at, those to come.
 *
 * `visible="draft,sent,paid"` shows only those steps, and the current one whatever it is; at a
 * value outside them — cancelled — none of them is passed.
 * `clickable="1"` lets the user move to a step, where the view edits the field.
 */
export class StatusbarWidget extends SelectionWidget {
    static override template = "web.StatusbarWidget";

    override props = props({ ...widgetProps });

    get steps(): Choice[] {
        const visible = (this.props.attrs as Record<string, string>).visible?.split(",").map((key) => key.trim());
        return this.choices.filter(([key]) => visible === undefined || visible.includes(key) || key === this.value);
    }

    /** Where the field stands among all its values: steps before it are passed. */
    get position(): number {
        return this.choices.findIndex(([key]) => key === this.value);
    }

    /** Whether the field is at one of the steps `visible` lists: a cancelled order is not. */
    get onTheWay(): boolean {
        const visible = (this.props.attrs as Record<string, string>).visible?.split(",").map((key) => key.trim());
        return visible === undefined || visible.includes(this.value as string);
    }

    stateOf(step: Choice): "passed" | "current" | "coming" {
        const at = this.choices.findIndex(([key]) => key === step[0]);
        if (at === this.position) {
            return "current";
        }
        return this.onTheWay && this.position >= 0 && at < this.position ? "passed" : "coming";
    }

    get clickable(): boolean {
        return this.editable && (this.props.attrs as Record<string, string>).clickable === "1";
    }
}

widgets.add("statusbar", StatusbarWidget);
