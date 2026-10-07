import { Component, type ComponentClass, props, t } from "trame";
import type { Values } from "@web/core/orm";
import type { Column } from "@web/views/view";
import type { CompiledCard } from "./card_compiler";

/** What shows cards — the strip beside a form, a kanban — as their templates read it. */
export interface CardHost {
    props: { resModel: string };
    holds(record: Values, expression: string): boolean;
    widgetFor(column: Column): ComponentClass;
    /** Change a field of a card's record from the card: a view whose cards say `quick_edit`. */
    quickEdit?(record: Values, column: Column, value: unknown): void;
    /** Open the editor of such a field, under what was clicked. */
    openQuickEdit?(record: Values, column: Column, event: MouseEvent): void;
}

/** What the template of a card reads: the view showing it, the record and the card's fields. */
export class CardBody extends Component {
    props = props({
        strip: t.any<CardHost>(),
        record: t.any<Values>(),
        card: t.any<CompiledCard>(),
    });

    get __strip(): CardHost {
        return this.props.strip as CardHost;
    }
}
