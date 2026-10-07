import { Component, type ComponentClass, computed, effect, inject, load, props, resource, t } from "trame";
import { Breadcrumb, listOf } from "@web/core/breadcrumb";
import { and } from "@web/core/domain";
import type { ActionDescription } from "@web/core/menus";
import { type Domain, Orm } from "@web/core/orm";
import { listKey, type Route, Router } from "@web/core/router";
import { RecordStrip } from "@web/views/strip/record_strip";
import { viewKinds } from "@web/views/view";

/**
 * The action open: where it is, its title, and one of its views — the kind the route asks for
 * when the action offers it, else the first a kind is registered for.
 *
 * On a view of one record, the breadcrumb ends with the record, and the action leads back to the
 * list; the record's view shows its own title. The records left on the way to it come between,
 * each leading back to itself; leaving the records for a list forgets them.
 *
 * A record shows beside the list it was opened from, in a strip
 * ([`RecordStrip`](../views/strip/record_strip.ts)) — that of its own action when it was opened
 * otherwise, if the action lists its records. The list stays as the user opens other records from
 * the record; a list of several opened from it shows beside it too. Which list that is, the
 * breadcrumb knows. A route's `ids` narrow the action to those records.
 */
export class ActionManager extends Component {
    static template = "web.ActionManager";
    static components = { RecordStrip };

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
    @inject(Orm) orm!: Orm;

    /** The records the view shows: the action's, narrowed to the route's `ids`. */
    get domain(): Domain {
        return narrowed(this.props.action.domain, this.router.route);
    }

    /** The list shown beside the view, if one. */
    @computed get stripRoute(): Route | null {
        if (this.isRecord) {
            return this.breadcrumb.strip ?? (this.switchable.length ? listOf(this.router.route) : null);
        }
        return this.breadcrumb.trail.length ? this.breadcrumb.strip : null;
    }

    /** The action of the list beside, loaded when it is not the one open. */
    @resource accessor stripAction: ActionDescription | null = load(
        () => this.stripRoute?.action ?? null,
        (name) => {
            const own = this.props.action;
            if (name === null || name === own.xml_id || name === String(own.id)) {
                return Promise.resolve(name === null ? null : own);
            }
            return this.orm.call<ActionDescription>("action", "load", [], { xml_id: name });
        },
    );

    get stripDomain(): Domain {
        const route = this.stripRoute;
        return route === null || !this.stripAction ? [] : narrowed(this.stripAction.domain, route);
    }

    get stripKey(): string {
        return this.stripRoute === null ? "" : (listKey(this.stripRoute) ?? "");
    }

    /** The record shown, when it is one of the list beside. */
    get stripSelected(): number | null {
        const route = this.router.route;
        return listKey(listOf(route)) === this.stripKey ? route.id : null;
    }

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

    /** Whether the view shows beside a list. */
    get besideList(): boolean {
        return this.stripRoute !== null;
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

/** An action's domain, narrowed to the records a route names, if it does. */
function narrowed(domain: readonly unknown[], route: Route): Domain {
    return route.ids ? and([[...domain], [["id", "in", route.ids]]]) : [...domain];
}
