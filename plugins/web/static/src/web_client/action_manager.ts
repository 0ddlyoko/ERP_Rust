import { Component, type ComponentClass, computed, effect, inject, load, loading, props, resource, t } from "trame";
import { Breadcrumb, type Crumb, listOf } from "@web/core/breadcrumb";
import { and } from "@web/core/domain";
import type { ActionDescription } from "@web/core/menus";
import { type Domain, Orm } from "@web/core/orm";
import { Models } from "@web/core/models";
import { byOf, listKey, type Route, Router } from "@web/core/router";
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
    @inject(Models) models!: Models;

    /** The records the view shows: the action's, narrowed to the route's `ids` or `by`. */
    get domain(): Domain {
        return narrowed(this.props.action.domain, this.router.route);
    }

    /** What a record created here starts with: the action's defaults, and the record of `by`, by its name once read. */
    get defaults(): Record<string, unknown> {
        const by = byOf(this.router.route);
        if (by === null) {
            return { ...this.props.action.defaults };
        }
        const name = loading(() => this.byName) ? null : this.byName;
        return { ...this.props.action.defaults, [by.field]: name === null ? by.id : [by.id, name] };
    }

    /** The name of the record of `by`, which names the list: the project of a board. */
    @resource accessor byName: string | null = load(
        () => ({ by: this.router.route.by ?? null, model: this.props.action.model }),
        async ({ by, model }) => {
            const of = byOf({ action: null, view: null, id: null, by });
            if (of === null) {
                return null;
            }
            const relation = (await this.models.fields(model))[of.field]?.relation;
            if (!relation) {
                return null;
            }
            const [[, name] = [of.id, null]] = await this.orm.names(relation, [of.id]);
            return name;
        },
    );

    /** What the list shown is called: the record of `by`, else the action. */
    get listName(): string {
        return (loading(() => this.byName) ? null : this.byName) ?? this.props.action.name;
    }

    @effect nameListInBreadcrumb(): () => void {
        this.breadcrumb.nameList(this.isRecord ? null : this.listName);
        return () => {
            this.breadcrumb.nameList(null);
        };
    }

    /** The crumbs shown, by position: a first one of the action's whole list is the root link. */
    get crumbs(): { crumb: Crumb; at: number }[] {
        return this.breadcrumb.trail
            .map((crumb, at) => ({ crumb, at }))
            .filter(({ crumb, at }) => !(at === 0 && crumb.route.id === null && !crumb.route.ids && !crumb.route.by));
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

    /** Back to the list the trail starts from, in the view it was left in — list or board. */
    backToList(): void {
        const back = this.breadcrumb.trail[0]?.route ?? this.breadcrumb.strip ?? listOf(this.router.route);
        this.router.go({ ...back, view: back.view === "form" ? null : back.view, id: null });
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

    /** Whether the action also shows its records one by one, to switch to from its lists. */
    get offersForm(): boolean {
        return this.props.action.views.includes("form") && viewKinds.has("form");
    }

    /** Switch to the form: the first record of the list shown, the list beside it. */
    async switchToForm(): Promise<void> {
        const [first] = await this.orm.search(this.props.action.model, this.domain, { limit: 1 });
        if (first !== undefined) {
            await this.router.go({ ...this.router.route, view: "form", id: first });
        }
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
    const by = byOf(route);
    const terms: Domain[] = [[...domain]];
    if (route.ids) {
        terms.push([["id", "in", route.ids]]);
    }
    if (by !== null) {
        terms.push([[by.field, "=", by.id]]);
    }
    return and(terms);
}
