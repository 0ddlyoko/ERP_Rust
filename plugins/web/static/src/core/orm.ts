import { inject, state } from "trame";
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
    inverse?: string;
    /** The records a relation offers to point to, as the field declares. */
    domain?: Domain;
    default?: unknown;
    /** Set on the field naming the model's records. */
    name_field?: boolean;
}

/** Records gathered by a value, as `read_group` answers. */
export interface Group {
    /** The value they share: a record as `[id, name]`, a period by its first day, `null` for none. */
    value: unknown;
    count: number;
    /** What the numbers asked for add up to, as text. */
    sums: Record<string, string>;
    /** The domain finding the group's records. */
    domain: Domain;
}

/** What changing a form's values changes in the fields computed from them, as `onchange` answers. */
export interface OnchangeAnswer {
    values: Values;
    /** The record's fields that could not be computed, with why. */
    errors: { field: string; message: string }[];
    /** By one2many or many2many: lines that exist by id, lines being created by draft number. */
    lines: Record<
        string,
        {
            updated: { id: number; values: Values }[];
            created: { draft: number; values: Values }[];
            errors: { id?: number; draft?: number; field: string; message: string }[];
        }
    >;
}

/** The records of any model, through the operations the protocol answers on every one of them. */
export class Orm {
    @inject(Rpc) rpc!: Rpc;

    @state accessor versions: Record<string, number> = {};

    /** Say records of a model changed out of the views' sight — a timer logged time — for those showing them to read them again. */
    touch(model: string): void {
        this.versions = { ...this.versions, [model]: this.versionOf(model) + 1 };
    }

    /** The records last saved from a view, for the others showing them to read them again. */
    @state accessor saved: { model: string; ids: number[]; at: number } | null = null;

    /** Say records were saved from a view: those showing them read them again, alone. */
    touchRecords(model: string, ids: number[]): void {
        this.saved = { model, ids, at: (this.saved?.at ?? 0) + 1 };
    }

    /** How many times records of the model were said to change; reading it follows the changes. */
    versionOf(model: string): number {
        return this.versions[model] ?? 0;
    }

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

    /** How many records each domain finds, in one call, in the order asked. */
    countEach(model: string, domains: Domain[]): Promise<number[]> {
        return this.rpc.call(`${model}.count`, { domains });
    }

    /**
     * The records matching `domain` gathered by `groupBy` — a field, `date_order:month` for a
     * date by period — counted, with the sums of `sums`; all of them in one group without it.
     */
    readGroup(model: string, domain: Domain, groupBy: string | null, sums: string[] = []): Promise<Group[]> {
        return this.rpc.call(`${model}.read_group`, { domain, group_by: groupBy ?? undefined, sums });
    }

    /**
     * What a new record starts with, for these fields: those its model gives a value, a record
     * pointed to as `[id, name]`.
     */
    defaultGet(model: string, fields: string[]): Promise<Values> {
        return this.rpc.call(`${model}.default_get`, { fields });
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

    /**
     * The records whose name holds `text`, whatever its case, as `[id, name]`: at most `limit`,
     * among those matching `domain`.
     */
    nameSearch(model: string, text: string, limit = 8, domain: Domain = []): Promise<[number, string][]> {
        return this.rpc.call(`${model}.name_search`, { text, limit, domain });
    }

    /**
     * The fields a form computes from what the user changed — of `id`, or of a new record — as
     * the server would once saved; nothing is saved.
     */
    onchange(model: string, id: number | undefined, values: Values): Promise<OnchangeAnswer> {
        return this.rpc.call(`${model}.onchange`, { id, values });
    }

    /** Create a record from its name alone; resolves with it as `[id, name]`. */
    nameCreate(model: string, text: string): Promise<[number, string]> {
        return this.rpc.call(`${model}.name_create`, { text });
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
