import { inject, load, resource } from "trame";
import { Orm } from "./orm";

/** What opening an entry shows: a model's records, in these kinds of views, the first one shown. */
export interface ActionDescription {
    readonly id: number;
    /** What a link names the action by: `base.action_users`. */
    readonly xml_id: string | null;
    readonly name: string;
    readonly model: string;
    readonly views: readonly string[];
    readonly domain: readonly unknown[];
}

/** An entry of the menu, as the server shows it to this user. */
export interface MenuEntry {
    readonly id: number;
    readonly name: string;
    readonly action: ActionDescription | null;
    readonly children: readonly MenuEntry[];
}

/**
 * The menus the user sees: loaded the first time something reads them, then kept for the page.
 *
 * Nothing it reads can change, so it is never loaded again on its own; `refresh(menus.tree)` does,
 * for whatever changes the menus, such as installing a plugin.
 */
export class Menus {
    @inject(Orm) orm!: Orm;

    @resource accessor tree: MenuEntry[] = load(() => this.orm.call<MenuEntry[]>("menu", "tree"));
}

/** Every action of a tree, depth first, in the order the menus show them. */
export function actionsOf(entries: readonly MenuEntry[]): ActionDescription[] {
    return entries.flatMap((entry) => [...(entry.action ? [entry.action] : []), ...actionsOf(entry.children)]);
}

/** Whether an entry, or one under it, opens this action. */
export function leadsTo(entry: MenuEntry, actionId: number): boolean {
    return entry.action?.id === actionId || entry.children.some((child) => leadsTo(child, actionId));
}

/** Whether an entry is this one, or holds it. */
export function holds(entry: MenuEntry, menuId: number): boolean {
    return entry.id === menuId || entry.children.some((child) => holds(child, menuId));
}

/** The first entry of a module opening an action, depth first. */
export function firstEntryOf(module: MenuEntry): MenuEntry | null {
    for (const child of module.children) {
        if (child.action !== null) {
            return child;
        }
        const below = firstEntryOf(child);
        if (below !== null) {
            return below;
        }
    }
    return null;
}

/** The top entry — the module — under which an action is opened. */
export function moduleOf(entries: readonly MenuEntry[], actionId: number): MenuEntry | null {
    return entries.find((entry) => leadsTo(entry, actionId)) ?? null;
}

/** The module holding an action on `model`, for an action no menu leads to. */
export function moduleOfModel(entries: readonly MenuEntry[], model: string): MenuEntry | null {
    return entries.find((entry) => actionsOf([entry]).some((action) => action.model === model)) ?? null;
}

/** Every entry opening an action, with the names leading to it: `Settings / Technical / Views`. */
export function pathsOf(entries: readonly MenuEntry[], above: string[] = []): { entry: MenuEntry; path: string }[] {
    return entries.flatMap((entry) => {
        const names = [...above, entry.name];
        const here = entry.action ? [{ entry, path: names.join(" / ") }] : [];
        return [...here, ...pathsOf(entry.children, names)];
    });
}

/** The action a menu opens on a model, as a link names it, to open one of its records with. */
export function actionFor(entries: readonly MenuEntry[], model: string | undefined): string | null {
    const action = actionsOf(entries).find((action) => action.model === model);
    return action === undefined ? null : (action.xml_id ?? String(action.id));
}
