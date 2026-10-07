import { computed, inject, load, loading, props, resource } from "trame";
import { Models } from "@web/core/models";
import { Orm } from "@web/core/orm";
import { type Choice, SelectionWidget } from "./selection_widget";
import { widgetProps, widgets } from "./widget";

/**
 * A selection as the steps it goes through, in its values' order: those passed, the one it is
 * at, those to come.
 *
 * `visible="draft,sent,paid"` shows only those steps, and the current one whatever it is; at a
 * value outside them — cancelled — none of them is passed.
 * `clickable="1"` lets the user move to a step, where the view edits the field.
 *
 * A many2one has the records of its model as steps, in their `sequence`: with `by="project"`,
 * those whose `project` is the record's — the columns of a task's board — and none while it has
 * no project. A step's key is then the record's id, as text. Its `domain` leaves out the records
 * not matching it — the one the record points to is shown whatever it is.
 */
export class StatusbarWidget extends SelectionWidget {
    static override template = "web.StatusbarWidget";

    override props = props({ ...widgetProps });

    @inject(Orm) orm!: Orm;
    @inject(Models) models!: Models;

    get isRecord(): boolean {
        return this.props.field.type === "ref";
    }

    /** The step the field is at: a selection's key, a record's id as text. */
    get key(): string | null {
        const value = this.value;
        if (this.isEmpty) {
            return null;
        }
        return Array.isArray(value) ? String(value[0]) : String(value);
    }

    /**
     * What the steps are read for — the model, and the record of `by` — as text: the same while
     * the user changes other fields, so the steps are not read again then.
     */
    @computed get stepsAsked(): string | null {
        if (!this.isRecord) {
            return null;
        }
        const by = (this.props.attrs as Record<string, string>).by;
        const of = by === undefined ? null : this.props.record[by];
        return JSON.stringify({ relation: this.props.field.relation ?? "", by, of: Array.isArray(of) ? of[0] : of, domain: this.domain });
    }

    /** The steps last read, shown while they are read again. */
    private shownRecords: Choice[] = [];

    /** The records a many2one may point to, as steps. */
    @resource accessor records: Choice[] = load(
        () => this.stepsAsked,
        async (text) => {
            const asked = text === null ? null : (JSON.parse(text) as { relation: string; by?: string; of: unknown; domain: unknown[] });
            if (asked === null || !asked.relation) {
                return [];
            }
            if (asked.by !== undefined && (asked.of === null || asked.of === undefined || asked.of === false)) {
                return [];
            }
            const fields = await this.models.fields(asked.relation);
            const name = Object.keys(fields).find((field) => fields[field].name_field) ?? "name";
            const domain = [...asked.domain, ...(asked.by === undefined ? [] : [[asked.by, "=", asked.of]])];
            const order = "sequence" in fields ? ["sequence", "id"] : ["id"];
            const rows = await this.orm.searchRead(asked.relation, domain, [name], { order });
            return rows.map((row): Choice => [String(row.id), String(row[name] ?? `#${row.id}`)]);
        },
    );

    override get choices(): Choice[] {
        if (!this.isRecord) {
            return super.choices;
        }
        if (!loading(() => this.records)) {
            this.shownRecords = this.records ?? [];
        }
        const records = this.shownRecords;
        const key = this.key;
        if (key === null || records.some(([known]) => known === key)) {
            return records;
        }
        const value = this.value;
        return [...records, [key, Array.isArray(value) ? String(value[1] ?? `#${key}`) : `#${key}`]];
    }

    override choose(key: string): void {
        if (!this.isRecord) {
            super.choose(key);
            return;
        }
        const label = this.choices.find(([known]) => known === key)?.[1] ?? null;
        this.props.onChange?.(key === "" ? null : [Number(key), label]);
    }

    get steps(): Choice[] {
        const visible = (this.props.attrs as Record<string, string>).visible?.split(",").map((key) => key.trim());
        return this.choices.filter(([key]) => visible === undefined || visible.includes(key) || key === this.key);
    }

    /** Where the field stands among all its values: steps before it are passed. */
    get position(): number {
        return this.choices.findIndex(([key]) => key === this.key);
    }

    /** Whether the field is at one of the steps `visible` lists: a cancelled order is not. */
    get onTheWay(): boolean {
        const visible = (this.props.attrs as Record<string, string>).visible?.split(",").map((key) => key.trim());
        return visible === undefined || visible.includes(this.key ?? "");
    }

    stateOf(step: Choice): "passed" | "current" | "coming" {
        const at = this.choices.findIndex(([key]) => key === step[0]);
        if (at === this.position) {
            return "current";
        }
        return this.onTheWay && this.position >= 0 && at < this.position ? "passed" : "coming";
    }

    get clickable(): boolean {
        return this.editable && (this.props.attrs as Record<string, string>).clickable === "1";
    }
}

widgets.add("statusbar", StatusbarWidget);
