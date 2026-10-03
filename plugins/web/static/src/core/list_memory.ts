/** A list as the user left it: which page, which rows selected, and the records it showed. */
export interface ListMemory {
    offset: number;
    selected: number[];
    /** The records of the page shown, in order: what a form steps through. */
    ids: number[];
    total: number;
}

const memories = new Map<string, ListMemory>();

/** The list of an action as the user left it, so going back to it finds it as it was. */
export function listMemory(action: string | null): ListMemory | undefined {
    return action === null ? undefined : memories.get(action);
}

export function rememberList(action: string | null, memory: ListMemory): void {
    if (action !== null) {
        memories.set(action, memory);
    }
}
