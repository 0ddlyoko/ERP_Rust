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

/** The top entry — the module — under which an action is opened. */
export function moduleOf(entries: readonly MenuEntry[], actionId: number): MenuEntry | null {
    return entries.find((entry) => leadsTo(entry, actionId)) ?? null;
}

/** Every entry opening an action, with the names leading to it: `Settings / Technical / Views`. */
export function pathsOf(entries: readonly MenuEntry[], above: string[] = []): { entry: MenuEntry; path: string }[] {
    return entries.flatMap((entry) => {
        const names = [...above, entry.name];
        const here = entry.action ? [{ entry, path: names.join(" / ") }] : [];
        return [...here, ...pathsOf(entry.children, names)];
    });
}
