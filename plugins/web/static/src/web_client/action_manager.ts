import { Component, type ComponentClass, computed, props, t } from "trame";
import type { ActionDescription } from "../core/menus";
import { viewKinds } from "../views/view";

/** The action open: its title, and the first of its views a kind is registered for. */
export class ActionManager extends Component {
    static template = "web.ActionManager";

    props = props({
        action: t.any<ActionDescription>(),
    });

    @computed get view(): ComponentClass | null {
        const kind = this.props.action.views.find((kind) => viewKinds.has(kind));
        return kind === undefined ? null : viewKinds.get(kind);
    }
}
