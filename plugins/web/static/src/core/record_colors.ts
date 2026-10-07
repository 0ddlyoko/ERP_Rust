import { inject, state } from "trame";
import { Models } from "./models";
import { Orm } from "./orm";

/**
 * The colour records have, for a model that gives them one in a `color` field — tags — read
 * together: the colours a page asks for at once are read in one call per model, rather than one
 * per record shown.
 */
export class RecordColors {
    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;

    /** Raised once colours were read, so that what shows them shows them. */
    @state accessor read = 0;

    private known = new Map<string, Map<number, string | null>>();
    private asked = new Map<string, Set<number>>();
    private scheduled = false;

    /** A record's colour; `null` until read, or when it has none. */
    colorOf(model: string, id: number): string | null {
        void this.read;
        const known = this.known.get(model);
        if (known?.has(id)) {
            return known.get(id) ?? null;
        }
        const asked = this.asked.get(model) ?? new Set<number>();
        asked.add(id);
        this.asked.set(model, asked);
        if (!this.scheduled) {
            this.scheduled = true;
            queueMicrotask(() => void this.readAsked());
        }
        return null;
    }

    private async readAsked(): Promise<void> {
        this.scheduled = false;
        const asked = this.asked;
        this.asked = new Map();
        for (const [model, ids] of asked) {
            const known = this.known.get(model) ?? new Map<number, string | null>();
            this.known.set(model, known);
            const wanted = [...ids].filter((id) => !known.has(id));
            for (const id of wanted) {
                known.set(id, null);
            }
            const fields = await this.models.fields(model);
            if (wanted.length === 0 || !("color" in fields)) {
                continue;
            }
            const rows = await this.orm.read(model, wanted, ["color"]);
            for (const row of rows) {
                known.set(row.id as number, typeof row.color === "string" ? row.color : null);
            }
        }
        this.read += 1;
    }
}
