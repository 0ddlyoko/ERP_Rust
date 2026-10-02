import { Component, type ComponentClass, computed, props, t } from "trame";
import type { ActionDescription } from "@web/core/menus";
import { viewKinds } from "@web/views/view";

/**
 * The action open: where it is, its title, and one of its views — the kind the route asks for
 * when the action offers it, else the first a kind is registered for.
 */
export class ActionManager extends Component {
    static template = "web.ActionManager";

    props = props({
        action: t.any<ActionDescription>(),
        /** The module the action is opened from, first in the breadcrumb. */
        module: t.string().default(""),
        /** The kind of view asked for: `list`, `form`. */
        view: t.string().orNull().default(null),
        /** The record shown, by a view showing one. */
        resId: t.number().orNull().default(null),
    });

    @computed get kind(): string | null {
        const offered = this.props.action.views.filter((kind) => viewKinds.has(kind));
        const asked = this.props.view;
        return asked !== null && offered.includes(asked) ? asked : (offered[0] ?? null);
    }

    @computed get view(): ComponentClass | null {
        return this.kind === null ? null : viewKinds.get(this.kind);
    }

    /** One view per action, kind and record: another is made anew rather than changed. */
    get viewKey(): string {
        return `${this.props.action.id}/${this.kind}/${this.props.resId ?? ""}`;
    }
}
