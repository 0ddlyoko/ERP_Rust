import { state } from "trame";

/** What the breadcrumb shows after the action: the record a view has open, or nothing. */
export class Breadcrumb {
    @state accessor record: string | null = null;
}
