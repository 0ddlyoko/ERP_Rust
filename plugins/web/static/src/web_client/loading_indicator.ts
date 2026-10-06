import { Component, inject } from "trame";
import { Rpc } from "@web/core/rpc";

/**
 * A small mark in the bottom right corner while the server is asked something: `Loading`, or
 * `Loading (2)` for calls waiting side by side. It shows only once a call has lasted a moment,
 * so that the quick ones do not make it flicker.
 */
export class LoadingIndicator extends Component {
    static template = "web.LoadingIndicator";

    @inject(Rpc) rpc!: Rpc;

    get text(): string {
        return this.rpc.pending > 1 ? `Loading (${this.rpc.pending})` : "Loading";
    }
}
