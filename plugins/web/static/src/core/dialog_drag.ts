/** What a dialog is dragged by: its header, its title, or its own margin. */
const HANDLES = ".o_dialog > header, .o_dialog > .o_activity_dialog_head, .o_dialog h2, .o_dialog_message";
/** What keeps a press for itself rather than starting a drag. */
const INTERACTIVE = "button, input, select, textarea, a, label, [contenteditable], [role=button]";
/** How much of a dialog stays on the screen, in pixels, however far it is dragged. */
const KEPT_IN_SIGHT = 48;

/**
 * Every dialog may be moved about, pressed by its header, its title or its margin and dragged:
 * what it covers can then be read. Listened for once on the page, so a dialog of any plugin moves
 * alike, and it starts again where it was laid out when opened anew.
 */
export function installDialogDrag(): void {
    document.addEventListener("pointerdown", (event) => {
        const target = event.target as Element | null;
        if (event.button !== 0 || target === null || target.closest(INTERACTIVE) !== null) {
            return;
        }
        const dialog = target.closest<HTMLElement>(".o_dialog");
        if (dialog === null || (target !== dialog && target.closest(HANDLES) === null)) {
            return;
        }
        event.preventDefault();
        const startX = event.clientX - Number(dialog.dataset.dragX ?? 0);
        const startY = event.clientY - Number(dialog.dataset.dragY ?? 0);
        const box = dialog.getBoundingClientRect();
        const laidX = box.left - Number(dialog.dataset.dragX ?? 0);
        const laidY = box.top - Number(dialog.dataset.dragY ?? 0);
        const move = (moved: PointerEvent): void => {
            const x = clamp(moved.clientX - startX, KEPT_IN_SIGHT - laidX - box.width, window.innerWidth - KEPT_IN_SIGHT - laidX);
            const y = clamp(moved.clientY - startY, -laidY, window.innerHeight - KEPT_IN_SIGHT - laidY);
            dialog.dataset.dragX = String(x);
            dialog.dataset.dragY = String(y);
            dialog.style.translate = `${x}px ${y}px`;
        };
        const stop = (): void => {
            dialog.classList.remove("o_dialog_dragging");
            window.removeEventListener("pointermove", move);
            window.removeEventListener("pointerup", stop);
        };
        dialog.classList.add("o_dialog_dragging");
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", stop);
    });
}

function clamp(value: number, low: number, high: number): number {
    return Math.min(Math.max(value, low), high);
}
