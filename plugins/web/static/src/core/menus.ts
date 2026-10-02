import { inject, load, resource } from "trame";
import type { Domain } from "./orm";
import { Orm } from "./orm";

/** What opening an entry shows: a model's records, in these kinds of views, the first one shown. */
export interface ActionDescription {
    id: number;
    /** What a link names the action by: `base.action_users`. */
    xml_id: string | null;
    name: string;
    model: string;
    views: string[];
    domain: Domain;
}

/** An entry of the menu, as the server shows it to this user. */
export interface MenuEntry {
    id: number;
    name: string;
    action: ActionDescription | null;
    children: MenuEntry[];
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
