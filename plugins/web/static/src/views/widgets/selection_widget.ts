import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** A value of a selection: its key and its label. */
export type Choice = [string, string];

/**
 * A field holding an enum, by its key: shown by its label, chosen from a list where a view edits
 * it. A field that may be empty offers no value too. A key its field does not list — a value of
 * a plugin since removed — is shown as it is. `choices="draft,sent"` offers only those values,
 * the one held shown whatever it is: the others are the server's to set.
 */
export class SelectionWidget extends Widget {
    static override template = "web.SelectionWidget";

    override props = props({ ...widgetProps });

    get choices(): Choice[] {
        const offered = (this.props.attrs as Record<string, string>).choices?.split(",").map((key) => key.trim());
        const values: Choice[] = (this.props.field.values ?? []).map(([key, label]) => [key, label]);
        const choices = offered === undefined ? values : values.filter(([key]) => offered.includes(key));
        const key = this.value as string;
        return this.isEmpty || choices.some(([known]) => known === key) ? choices : [...choices, [key, key]];
    }

    override get text(): string {
        const values = this.props.field.values ?? [];
        return this.isEmpty ? "" : (values.find(([key]) => key === this.value)?.[1] ?? String(this.value));
    }

    /** Whether "no value" is offered: the field may be empty, or is. */
    get offersNothing(): boolean {
        return !this.props.field.required || this.isEmpty;
    }

    choose(key: string): void {
        this.props.onChange?.(key === "" ? null : key);
    }
}

widgets.add("selection", SelectionWidget);
