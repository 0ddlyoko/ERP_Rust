import type { Notifications } from "@web/core/notifications";
import type { Orm } from "@web/core/orm";
import type { Facet, Favorite } from "./search_model";

/** The searches the user saved on an action's records. */
export function favoritesOf(orm: Orm, action: string | null): Promise<Favorite[]> {
    return action === null ? Promise.resolve([]) : orm.call<Favorite[]>("saved_filter", "mine", [], { action });
}

/** Save a search under a name, the view opening with it if asked; whether it was saved. */
export async function saveFavorite(
    orm: Orm,
    notifications: Notifications,
    action: string | null,
    name: string,
    facets: Facet[],
    isDefault: boolean,
): Promise<boolean> {
    if (action === null) {
        return false;
    }
    try {
        await orm.call("saved_filter", "save", [], { action, name, facets, is_default: isDefault });
        notifications.add("success", `Search "${name}" saved.`);
        return true;
    } catch (error) {
        notifications.add("danger", error instanceof Error ? error.message : String(error));
        return false;
    }
}

/** Forget a saved search. */
export async function forgetFavorite(orm: Orm, favorite: Favorite): Promise<void> {
    await orm.call("saved_filter", "forget", [], { id: favorite.id });
}
