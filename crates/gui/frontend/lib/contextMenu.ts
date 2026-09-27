/** Whether the target is somewhere text is typed, where the native menu's Cut, Copy and Paste are the point. */
const isEditable = (target: EventTarget | null): boolean =>
    target instanceof HTMLElement &&
    (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target.isContentEditable);

/**
 * Suppresses the webview's own context menu, whose "Reload", "Inspect Element" and "Download Image" belong
 * to a browser rather than to this application. Tauri leaves it on in every build, release included.
 *
 * Text fields keep it: there the menu is the platform's editing one, not the browser's.
 *
 * On `document` and without capture, so a menu a component draws itself - which calls `preventDefault`
 * on its own - is unaffected either way.
 *
 * Installed in `main.tsx` on production builds only, so `pnpm dev` still has Inspect Element.
 *
 * Returns the removal of the handler.
 */
export const suppressContextMenu = (): (() => void) => {
    const onContextMenu = (event: MouseEvent) => {
        if (!isEditable(event.target)) event.preventDefault();
    };

    document.addEventListener("contextmenu", onContextMenu);

    return () => document.removeEventListener("contextmenu", onContextMenu);
};
