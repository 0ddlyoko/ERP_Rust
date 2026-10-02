import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/** The records of a one2many or a many2many: how many there are. */
export class X2ManyWidget extends Widget {
    override props = props({ ...widgetProps });

    /** Chosen in a list of records, which comes later. */
    override get canEdit(): boolean {
        return false;
    }

    override get text(): string {
        const ids = Array.isArray(this.value) ? this.value : [];
        return ids.length === 1 ? "1 record" : `${ids.length} records`;
    }
}

widgets.add("x2many", X2ManyWidget);
