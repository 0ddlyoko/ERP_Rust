import { inject } from "trame";
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

/** The menus the user sees: asked for once, kept for the page. */
export class Menus {
    @inject(Orm) orm!: Orm;

    private known: Promise<MenuEntry[]> | undefined;

    tree(): Promise<MenuEntry[]> {
        if (this.known === undefined) {
            this.known = this.orm.call<MenuEntry[]>("menu", "tree");
            this.known.catch(() => {
                this.known = undefined;
            });
        }
        return this.known;
    }
}

/** Every action of a tree, depth first, in the order the menus show them. */
export function actionsOf(entries: readonly MenuEntry[]): ActionDescription[] {
    return entries.flatMap((entry) => [...(entry.action ? [entry.action] : []), ...actionsOf(entry.children)]);
}
