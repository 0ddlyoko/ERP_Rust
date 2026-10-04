/** The narrowest a column is dragged to, in pixels. */
const NARROWEST = 48;

/**
 * The widths of a table's columns once the user sized one by hand: each column shown, by name,
 * and the width of those around them that are not sized — a selection box, buttons.
 */
export interface ColumnWidths {
    columns: Record<string, number>;
    others: number;
}

/**
 * Follow the mouse from a column's resize handle, calling `apply` with the widths as it moves.
 *
 * The first time, every column takes the width it had: from then on each keeps its own, and an
 * empty column after them takes what is left. The click ending the drag is swallowed, so that a
 * header sorting its column on click does not sort it.
 */
export function dragColumn(
    event: MouseEvent,
    name: string,
    current: ColumnWidths | null,
    apply: (widths: ColumnWidths) => void,
): void {
    const header = event.currentTarget instanceof Element ? event.currentTarget.closest("tr") : null;
    if (header === null) {
        return;
    }
    event.preventDefault();
    event.stopPropagation();
    const widths = current ?? measure(header);
    const from = event.clientX;
    const start = widths.columns[name] ?? NARROWEST;
    const move = (moved: MouseEvent): void => {
        const width = Math.max(NARROWEST, Math.round(start + moved.clientX - from));
        apply({ ...widths, columns: { ...widths.columns, [name]: width } });
    };
    const stop = (): void => {
        document.removeEventListener("mousemove", move);
        document.removeEventListener("mouseup", stop);
        document.body.classList.remove("o_resizing");
        const swallow = (click: Event): void => {
            click.stopPropagation();
            click.preventDefault();
        };
        window.addEventListener("click", swallow, { capture: true, once: true });
        setTimeout(() => window.removeEventListener("click", swallow, { capture: true }));
    };
    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", stop);
    document.body.classList.add("o_resizing");
    apply(widths);
}

/** The width of a column sized by hand, as its header's style; nothing before any was. */
export function columnStyle(widths: ColumnWidths | null, name: string): string {
    const width = widths?.columns[name];
    return width === undefined ? "" : `width: ${width}px`;
}

/** The table's style once its columns are sized: as wide as they are, and at least its box. */
export function tableStyle(widths: ColumnWidths | null): string {
    if (widths === null) {
        return "";
    }
    const total = Object.values(widths.columns).reduce((sum, width) => sum + width, widths.others);
    return `width: max(100%, ${Math.ceil(total)}px)`;
}

/** The widths the header's columns have now, those named by `data-column` apart. */
function measure(header: Element): ColumnWidths {
    const columns: Record<string, number> = {};
    let others = 0;
    for (const cell of Array.from(header.children)) {
        const width = cell.getBoundingClientRect().width;
        const name = cell instanceof HTMLElement ? cell.dataset.column : undefined;
        if (name === undefined) {
            others += width;
        } else {
            columns[name] = Math.round(width);
        }
    }
    return { columns, others };
}
