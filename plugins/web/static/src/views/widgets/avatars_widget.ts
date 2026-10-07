import { props, state } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { type Choice, RecordSearch } from "./record_search";
import { Widget, widgetProps, widgets } from "./widget";

/** How many avatars show before the rest is counted. */
const SHOWN = 3;

/**
 * The records of a many2many as their initials, side by side, overlapping — the people a task is
 * given to — the first few, and how many more. Where a view edits it, all are shown: one is
 * removed with its cross, and one added with the button beside them, by searching.
 */
export class AvatarsWidget extends Widget {
    static override template = "web.AvatarsWidget";
    static components = { RecordSearch };

    override props = props({ ...widgetProps });

    @state accessor adding = false;

    /** The records held, as `[id, name]`. */
    get held(): [number, string][] {
        const value = this.value;
        if (!Array.isArray(value)) {
            return [];
        }
        return value.map((entry): [number, string] =>
            Array.isArray(entry) ? [Number(entry[0]), String(entry[1] ?? `#${entry[0]}`)] : [Number(entry), `#${entry}`],
        );
    }

    get people(): { id: number; name: string; initials: string; style: string }[] {
        return this.held.map(([id, name]) => ({ id, name, initials: initialsOf(name), style: avatarStyleOf(name) }));
    }

    readonly add = (choice: Choice): void => {
        this.adding = false;
        this.props.onChange?.([...this.held, choice]);
    };

    remove(id: number): void {
        this.props.onChange?.(this.held.filter(([held]) => held !== id));
    }

    get names(): string[] {
        const value = this.value;
        if (!Array.isArray(value)) {
            return [];
        }
        return value.map((entry) => (Array.isArray(entry) ? String(entry[1] ?? `#${entry[0]}`) : `#${entry}`));
    }

    get shown(): { name: string; initials: string; style: string }[] {
        return this.names.slice(0, SHOWN).map((name) => ({ name, initials: initialsOf(name), style: avatarStyleOf(name) }));
    }

    get more(): number {
        return Math.max(0, this.names.length - SHOWN);
    }

    override get text(): string {
        return this.names.join(", ");
    }

    override get canEdit(): boolean {
        return this.props.field.relation !== undefined;
    }
}

widgets.add("avatars", AvatarsWidget);
