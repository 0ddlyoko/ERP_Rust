import { Component, props, t } from "trame";
import type { Values } from "@web/core/orm";
import type { RecordStrip } from "./record_strip";
import type { CompiledCard } from "./card_compiler";

/** What the template of a card reads: the strip showing it, the record and the card's fields. */
export class CardBody extends Component {
    props = props({
        strip: t.any<RecordStrip>(),
        record: t.any<Values>(),
        card: t.any<CompiledCard>(),
    });

    get __strip(): RecordStrip {
        return this.props.strip as RecordStrip;
    }
}
