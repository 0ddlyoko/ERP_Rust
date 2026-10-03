import { inject } from "trame";
import { Rpc } from "./rpc";

/** A search domain, as the server reads it: `[["name", "=", "Blue"], ...]`. */
export type Domain = unknown[];

/** Which records of a search, and in which order: `order` as `["name asc", "id desc"]`. */
export interface Paging {
    limit?: number;
    offset?: number;
    order?: string[];
}

/** How records are read. */
export interface ReadOptions {
    /** Many2ones as `[id, name]` rather than the id alone; the name `null` when out of reach. */
    names?: boolean;
}

/** A record's values, by field name. */
export type Values = Record<string, unknown>;

/** How a field is shown and edited, as the server describes it. */
export interface FieldDescription {
    type: "string" | "integer" | "decimal" | "bool" | "date" | "datetime" | "ref" | "refs" | "selection";
    label: string;
    required: boolean;
    readonly: boolean;
    stored: boolean;
    relation?: string;
    relation_kind?: "many2one" | "one2many" | "many2many";
    values?: [string, string][];
    default?: unknown;
}

/** The records of any model, through the operations the protocol answers on every one of them. */
export class Orm {
    @inject(Rpc) rpc!: Rpc;

    search(model: string, domain: Domain = [], paging: Paging = {}): Promise<number[]> {
        return this.rpc.call(`${model}.search`, { domain, ...paging });
    }

    read(model: string, ids: number[], fields: string[], options: ReadOptions = {}): Promise<Values[]> {
        return this.rpc.call(`${model}.read`, { ids, fields, ...options });
    }

    /** Search and read in one call. */
    searchRead(
        model: string,
        domain: Domain,
        fields: string[],
        options: Paging & ReadOptions = {},
    ): Promise<Values[]> {
        return this.rpc.call(`${model}.read_matching`, { domain, fields, ...options });
    }

    /** The names of records, as `[id, name]` in the order asked; `null` for one out of reach. */
    names(model: string, ids: number[]): Promise<[number, string | null][]> {
        return this.rpc.call(`${model}.names`, { ids });
    }

    count(model: string, domain: Domain = []): Promise<number> {
        return this.rpc.call(`${model}.count`, { domain });
    }

    /** Create one record, or several; resolves with their ids. */
    create(model: string, values: Values | Values[]): Promise<number[]> {
        return this.rpc.call(`${model}.create`, { values });
    }

    write(model: string, ids: number[], values: Values): Promise<boolean> {
        return this.rpc.call(`${model}.write`, { ids, values });
    }

    /** Resolves with how many records were deleted. */
    delete(model: string, ids: number[]): Promise<number> {
        return this.rpc.call(`${model}.delete`, { ids });
    }

    /** The records whose name holds `text`, whatever its case, as `[id, name]`: at most `limit`. */
    nameSearch(model: string, text: string, limit = 8): Promise<[number, string][]> {
        return this.rpc.call(`${model}.name_search`, { text, limit });
    }

    /** The fields of a model the caller may see, all of them or those named. */
    fieldsGet(model: string, fields: string[] = []): Promise<Record<string, FieldDescription>> {
        return this.rpc.call(`${model}.fields_get`, { fields });
    }

    /** A method the model exposes with `#[erp(rpc)]`, on these records. */
    call<T = unknown>(model: string, method: string, ids: number[] = [], args: Values = {}): Promise<T> {
        return this.rpc.call(`${model}.${method}`, { ids, args });
    }
}
