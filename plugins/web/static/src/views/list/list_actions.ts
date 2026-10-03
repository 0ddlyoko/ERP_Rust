import { registry } from "trame";
import type { Orm } from "@web/core/orm";

/** What a list action works on: the records selected in a list of a model. */
export interface ListActionContext {
    orm: Orm;
    model: string;
    ids: number[];
}

/**
 * Something a list does with the records selected, whatever their model. Its `confirm`, if any,
 * is asked first, given how many records are selected.
 */
export interface ListAction {
    label: string;
    confirm?: (count: number) => string;
    run(context: ListActionContext): Promise<unknown>;
}

/**
 * The actions every list offers on its selection, beside the buttons its view declares. A plugin
 * adds one here: `listActions.add("export", { label: "Export", run: ... })`.
 */
export const listActions = registry.category<ListAction>("list_actions");

listActions.add(
    "delete",
    {
        label: "Delete",
        confirm: (count) => (count === 1 ? "Delete this record?" : `Delete these ${count} records?`),
        run: ({ orm, model, ids }) => orm.delete(model, ids),
    },
    { sequence: 100 },
);
