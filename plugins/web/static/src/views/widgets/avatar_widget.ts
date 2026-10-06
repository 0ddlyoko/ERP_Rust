import { props } from "trame";
import { avatarStyleOf, initialsOf } from "@web/core/avatar";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * A record, or a name, as the initials of its name on a tint of its own: the same name always
 * shows the same way, so a contact is known at a glance. Shown, never edited.
 */
export class AvatarWidget extends Widget {
    static override template = "web.AvatarWidget";

    override props = props({ ...widgetProps });

    /** The name the avatar stands for: a record's, or the text itself. */
    get name(): string {
        const value = this.value;
        if (Array.isArray(value)) {
            return String(value[1] ?? "");
        }
        return this.isEmpty ? "" : String(value);
    }

    get initials(): string {
        return initialsOf(this.name);
    }

    get style(): string {
        return avatarStyleOf(this.name);
    }

    override get canEdit(): boolean {
        return false;
    }
}

widgets.add("avatar", AvatarWidget);
