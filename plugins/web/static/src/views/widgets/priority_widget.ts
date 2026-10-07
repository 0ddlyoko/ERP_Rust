import { props } from "trame";
import { SelectionWidget } from "./selection_widget";
import { widgetProps, widgets } from "./widget";

/**
 * A priority as stars: none lit for the first value — normal — then one more for each value
 * after it. Where a view edits it, a star is pressed to raise the priority to it, the lit one
 * highest to lower it back.
 */
export class PriorityWidget extends SelectionWidget {
    static override template = "web.PriorityWidget";

    override props = props({ ...widgetProps });

    /** The values above the first, one star each. */
    get stars(): { key: string; label: string; lit: boolean }[] {
        const choices = this.choices;
        const at = choices.findIndex(([key]) => key === this.value);
        return choices.slice(1).map(([key, label], index) => ({ key, label, lit: index < at }));
    }

    press(key: string): void {
        const choices = this.choices;
        const at = choices.findIndex(([choice]) => choice === key);
        const lowered = this.value === key ? choices[at - 1]?.[0] : key;
        this.choose(lowered ?? choices[0]?.[0] ?? "");
    }
}

widgets.add("priority", PriorityWidget);
