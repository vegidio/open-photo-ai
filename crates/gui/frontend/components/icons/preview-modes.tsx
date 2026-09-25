import { createLucideIcon, type LucideIconData } from "lucide-react";

/*
 * The three preview-comparison modes, which are the only icons in this application that lucide does
 * not have: they are diagrams of what each mode does - one pane, two panes, one pane split behind a
 * handle - rather than pictures of a thing with a name. The Wails app drew them as custom SVG for the
 * same reason.
 *
 * `createLucideIcon` rather than hand-written `<svg>`: it is what fixes the 24x24 viewBox,
 * `fill="none"`, `stroke="currentColor"` and the round caps and joins, and - the part that matters -
 * it reads `size`, `color` and `strokeWidth` from the same `LucideProvider` context every real lucide
 * icon reads, so these sit in the toggle group beside `Plus` and `ChevronsUp` at the app's stroke 1.5
 * instead of carrying their own copy of it. The paths are the design's.
 *
 * The icon data is declared separately and exported for the same reason lucide exports its own
 * `__iconData`: it is the only way to assert anything about an icon's nodes once `createLucideIcon`
 * has closed over them. The `key` on each is required rather than decorative - lucide renders the
 * nodes with `node.map(([tag, attrs]) => createElement(tag, attrs))` and supplies no key itself, so an
 * icon whose nodes carry none renders correctly and logs a React warning on every mount.
 */

/** One pane: the enhanced result alone. */
export const previewFullIconData: LucideIconData = {
    name: "preview-full",
    size: 24,
    node: [["path", { d: "M3 6a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z", key: "pane" }]],
};

/** Two panes side by side: original and enhanced. */
export const previewSideIconData: LucideIconData = {
    name: "preview-side",
    size: 24,
    node: [
        ["path", { d: "M3 5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z", key: "left" }],
        ["path", { d: "M13 5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2Z", key: "right" }],
    ],
};

/** One pane, the two images overlaid behind a draggable handle. */
export const previewSplitIconData: LucideIconData = {
    name: "preview-split",
    size: 24,
    node: [
        ["path", { d: "M12 3v18", key: "handle" }],
        ["path", { d: "M5 3h3a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2", key: "left" }],
        ["path", { d: "M16 7h3a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-3", key: "right" }],
    ],
};

export const PreviewFullIcon = createLucideIcon(previewFullIconData);
export const PreviewSideIcon = createLucideIcon(previewSideIconData);
export const PreviewSplitIcon = createLucideIcon(previewSplitIconData);
