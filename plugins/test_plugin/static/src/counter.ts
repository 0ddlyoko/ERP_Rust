import { twice } from "./util/format";
import type { Shape } from "./types";

declare function state(target: unknown, context: ClassAccessorDecoratorContext): void;

/// A class the way Trame declares one: a standard decorator on an auto-accessor.
export class Counter {
    @state accessor count: number = 0;

    get doubled(): number {
        return twice(this.count);
    }
}

export const shape: Shape | undefined = undefined;
