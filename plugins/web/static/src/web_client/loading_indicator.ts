import { Component, inject } from "trame";
import { Rpc } from "@web/core/rpc";

/**
 * A thin bar running along the top of the page while the server is asked something — a record
 * opening, a list read. It shows only once a call has lasted a moment, so that the quick ones do
 * not make it flicker; how many calls wait is said to assistive technologies.
 */
export class LoadingIndicator extends Component {
    static template = "web.LoadingIndicator";

    @inject(Rpc) rpc!: Rpc;

    get text(): string {
        return this.rpc.pending > 1 ? `Loading (${this.rpc.pending})` : "Loading";
    }
}
