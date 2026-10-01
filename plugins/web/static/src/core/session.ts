/** What the server writes into `#session_info` for the page it renders. */
export interface SessionInfo {
    uid: number;
    name: string;
    login: string;
    groups: string[];
    csrf_token: string;
}

/** Who is logged in, as the page was rendered for them. */
export class Session {
    readonly uid: number;
    readonly name: string;
    readonly login: string;
    readonly csrfToken: string;
    private readonly groups: ReadonlySet<string>;

    constructor(info: SessionInfo) {
        this.uid = info.uid;
        this.name = info.name;
        this.login = info.login;
        this.csrfToken = info.csrf_token;
        this.groups = new Set(info.groups);
    }

    /** The session the server wrote into the page. */
    static fromPage(page: Document = document): Session {
        const element = page.getElementById("session_info");
        if (element === null || !element.textContent) {
            throw new Error("The page holds no #session_info: it was not rendered for a session");
        }
        return new Session(JSON.parse(element.textContent) as SessionInfo);
    }

    /** Whether the user is in a group, named by its external identifier: `base.group_admin`. */
    hasGroup(externalId: string): boolean {
        return this.groups.has(externalId);
    }
}
