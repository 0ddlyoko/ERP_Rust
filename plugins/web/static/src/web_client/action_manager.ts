import { Component, type ComponentClass, computed, effect, inject, props, t } from "trame";
import { Breadcrumb } from "@web/core/breadcrumb";
import type { ActionDescription } from "@web/core/menus";
import { Router } from "@web/core/router";
import { viewKinds } from "@web/views/view";

/**
 * The action open: where it is, its title, and one of its views — the kind the route asks for
 * when the action offers it, else the first a kind is registered for.
 *
 * On a view of one record, the breadcrumb ends with the record, and the action leads back to the
 * list; the record's view shows its own title. The records left on the way to it come between,
 * each leading back to itself; leaving the records for a list forgets them.
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

    @inject(Breadcrumb) breadcrumb!: Breadcrumb;
    @inject(Router) router!: Router;

    get isRecord(): boolean {
        return this.kind === "form";
    }

    /** The action the breadcrumb starts from: the one the first record left was under. */
    get rootAction(): string {
        return this.breadcrumb.trail[0]?.action ?? this.props.action.name;
    }

    backToList(): void {
        const first = this.breadcrumb.trail[0];
        this.router.go({ ...(first?.route ?? this.router.route), view: null, id: null });
    }

    backTo(index: number): void {
        void this.breadcrumb.back(index);
    }

    @effect nameInBreadcrumb(): () => void {
        this.breadcrumb.action = this.props.action.name;
        return () => {
            this.breadcrumb.action = null;
        };
    }

    @computed get kind(): string | null {
        const offered = this.props.action.views.filter((kind) => viewKinds.has(kind));
        const asked = this.props.view;
        return asked !== null && offered.includes(asked) ? asked : (offered[0] ?? null);
    }

    /** The views of several records the action offers, to switch between: list, kanban. */
    get switchable(): string[] {
        return this.props.action.views.filter((kind) => kind !== "form" && viewKinds.has(kind));
    }

    switchTo(kind: string): void {
        this.router.go({ ...this.router.route, view: kind, id: null });
    }

    @computed get view(): ComponentClass | null {
        return this.kind === null ? null : viewKinds.get(this.kind);
    }

    /** One view per action, kind and record: another is made anew rather than changed. */
    get viewKey(): string {
        return `${this.props.action.id}/${this.kind}/${this.props.resId ?? ""}`;
    }
}
