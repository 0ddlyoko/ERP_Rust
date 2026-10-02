import { inject } from "trame";
import { type FieldDescription, Orm } from "./orm";

/** The fields of a model, by name. */
export type Fields = Readonly<Record<string, FieldDescription>>;

/** What the client knows of each model's fields: asked for once per model, kept for the page. */
export class Models {
    @inject(Orm) orm!: Orm;

    private readonly known = new Map<string, Promise<Fields>>();

    /**
     * Every field of a model the user may see.
     *
     * Views asking at the same time share one call. A call that failed is forgotten, so the next
     * view asks again rather than inheriting the failure.
     */
    fields(model: string): Promise<Fields> {
        let fields = this.known.get(model);
        if (fields === undefined) {
            fields = this.orm.fieldsGet(model);
            this.known.set(model, fields);
            fields.catch(() => this.known.delete(model));
        }
        return fields;
    }
}
