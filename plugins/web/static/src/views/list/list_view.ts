import { load, props, resource, t } from "trame";
import type { Values } from "../../core/orm";
import { View, viewProps } from "../view";

/** Records of a model as rows, one column per field shown. */
export class ListView extends View {
    static template = "web.ListView";

    override get kind(): string {
        return "list";
    }

    override props = props({
        ...viewProps,
        /** How many rows at most. */
        limit: t.number().default(80),
    });

    @resource accessor records: Values[] = load(
        () => ({
            model: this.props.resModel,
            fields: this.columns.map((column) => column.name),
            domain: [...this.props.domain],
            limit: this.props.limit,
        }),
        ({ model, fields, domain, limit }) =>
            this.orm.searchRead(model, domain, fields, { limit, names: true }),
    );
}
