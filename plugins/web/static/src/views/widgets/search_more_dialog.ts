import { Component, effect, inject, props, t } from "trame";
import { Models } from "@web/core/models";
import { Orm } from "@web/core/orm";
import { viewKinds } from "@web/views/view";
import type { Choice } from "./record_search";

/**
 * More records of a model than a field's list offers, in a dialog: searched, filtered and grouped
 * as a list of them is. Choosing a row picks its record; selecting rows first, those selected are
 * picked together — or only one, for a field holding one.
 */
export class SearchMoreDialog extends Component {
    static template = "web.SearchMoreDialog";

    props = props({
        model: t.string(),
        domain: t.array(t.any()).default([]),
        /** Records not offered: those already chosen. */
        exclude: t.array(t.number()).default([]),
        /** What was typed in the field: the search starts with it. */
        text: t.string().default(""),
        /** Whether several records may be picked at once. */
        many: t.boolean().default(false),
        onPick: t.func<(choices: Choice[]) => void>(),
        onClose: t.func<() => void>(),
    });

    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;

    get list(): unknown {
        return viewKinds.get("list");
    }

    get domain(): unknown[] {
        const domain = [...this.props.domain];
        return this.props.exclude.length ? [...domain, ["id", "not in", [...this.props.exclude]]] : domain;
    }

    /** Escape closes the dialog wherever the focus is, unless something in it took the key. */
    @effect closeOnEscape(): () => void {
        const close = (event: KeyboardEvent): void => {
            if (event.key === "Escape" && !event.defaultPrevented) {
                this.props.onClose();
            }
        };
        window.addEventListener("keydown", close);
        return () => window.removeEventListener("keydown", close);
    }

    /** The records chosen, by name, handed to the field; the dialog closes. */
    readonly choose = async (ids: number[]): Promise<void> => {
        const names = new Map(await this.orm.names(this.props.model, ids));
        this.props.onPick(ids.map((id): Choice => [id, names.get(id) ?? ""]));
        this.props.onClose();
    };
}
