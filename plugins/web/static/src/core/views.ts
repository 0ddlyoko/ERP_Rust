import { inject } from "trame";
import { Orm } from "./orm";

/** What the client knows of each view: its final XML, asked for once per model and kind. */
export class Views {
    @inject(Orm) orm!: Orm;

    private readonly known = new Map<string, Promise<string>>();

    /**
     * The view shown for a model's records of one kind, `list` or `form`: its XML, every extension
     * applied.
     *
     * Views asking at the same time share one call; one that failed is forgotten, so the next asks
     * again.
     */
    arch(model: string, kind: string): Promise<string> {
        const key = `${model}/${kind}`;
        let arch = this.known.get(key);
        if (arch === undefined) {
            arch = this.orm.call<string>("view", "load", [], { model, kind });
            this.known.set(key, arch);
            arch.catch(() => this.known.delete(key));
        }
        return arch;
    }
}
