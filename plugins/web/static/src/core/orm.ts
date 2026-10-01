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

/** A record's values, by field name. */
export type Values = Record<string, unknown>;

/** The records of any model, through the operations the protocol answers on every one of them. */
export class Orm {
    @inject(Rpc) rpc!: Rpc;

    search(model: string, domain: Domain = [], paging: Paging = {}): Promise<number[]> {
        return this.rpc.call(`${model}.search`, { domain, ...paging });
    }

    read(model: string, ids: number[], fields: string[]): Promise<Values[]> {
        return this.rpc.call(`${model}.read`, { ids, fields });
    }

    /** Search and read in one call. */
    searchRead(model: string, domain: Domain, fields: string[], paging: Paging = {}): Promise<Values[]> {
        return this.rpc.call(`${model}.read_matching`, { domain, fields, ...paging });
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

    /** A method the model exposes with `#[erp(rpc)]`, on these records. */
    call<T = unknown>(model: string, method: string, ids: number[] = [], args: Values = {}): Promise<T> {
        return this.rpc.call(`${model}.${method}`, { ids, args });
    }
}
