import { inject, state, untrack } from "trame";
import { Session } from "./session";

/** What the server answered instead of a result: the JSON-RPC error, as it was sent. */
export class RpcError extends Error {
    constructor(
        readonly code: number,
        message: string,
        readonly data?: unknown,
    ) {
        super(message);
        this.name = "RpcError";
    }
}

/** The session is over: expired, revoked, or no longer the one the page was rendered for. */
export class SessionExpired extends RpcError {
    constructor(code: number, message: string, data?: unknown) {
        super(code, message, data);
        this.name = "SessionExpired";
    }
}

/** Codes the server answers with when the call carries no live session of the page's. */
export const SESSION_ENDED_CODES: readonly number[] = [-32001, -32002];

interface RawError {
    code: number;
    message: string;
    data?: unknown;
}

/** The error a JSON-RPC answer carries, as the class a caller can tell apart. */
export function toError(raw: RawError): RpcError {
    if (SESSION_ENDED_CODES.includes(raw.code)) {
        return new SessionExpired(raw.code, raw.message, raw.data);
    }
    return new RpcError(raw.code, raw.message, raw.data);
}

/** The body of a call to `/jsonrpc`. */
export function request(method: string, params: object, id: number): string {
    return JSON.stringify({ jsonrpc: "2.0", method, params, id });
}

/** Calls to the server's `/jsonrpc`, carried by the session cookie and the page's CSRF token. */
export class Rpc {
    @inject(Session) session!: Session;

    /** How many calls are waiting for their answer, for the page to show it is busy. */
    @state accessor pending = 0;

    private nextId = 1;

    /**
     * Call `model.method`, resolving with its result.
     *
     * A session that ended sends the browser to log in again, back to this page after; the call
     * still rejects, so nothing goes on as if it had succeeded.
     */
    async call<T = unknown>(method: string, params: object = {}): Promise<T> {
        // Counted out of whatever is tracking: a resource making the call must not reload on it.
        untrack(() => this.pending++);
        try {
            return await this.send<T>(method, params);
        } finally {
            untrack(() => this.pending--);
        }
    }

    private async send<T>(method: string, params: object): Promise<T> {
        const response = await fetch("/jsonrpc", {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/json", "X-CSRF-Token": this.session.csrfToken },
            body: request(method, params, this.nextId++),
        });
        let answer: { result?: T; error?: RawError };
        try {
            answer = await response.json();
        } catch {
            throw new RpcError(-32603, `The server answered ${response.status} without JSON`);
        }
        if (answer.error !== undefined) {
            const error = toError(answer.error);
            if (error instanceof SessionExpired) {
                logInAgain();
            }
            throw error;
        }
        return answer.result as T;
    }
}

/**
 * Log in again, then come back where the user was — the record or list the address's hash names,
 * which only the browser knows. Still logged in, as when the server restarted with another
 * signing secret, the login page sends them straight back, with a page holding a valid token.
 */
function logInAgain(): void {
    const back = window.location.pathname + window.location.search + window.location.hash;
    window.location.assign(`/login?redirect=${encodeURIComponent(back)}`);
}
