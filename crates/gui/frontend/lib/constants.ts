import type { Link } from "@/ipc/links";

/** The application's name. Not an i18n key: a brand name should read the same in every language. */
export const APP_NAME = "Open Photo AI";

/** The About dialog's copyright line. Not an i18n key, for the same reason as {@link APP_NAME}. */
export const APP_COPYRIGHT = "© 2025—2026, Vinicius Egidio";

/**
 * Labels for the About dialog's two links, keyed by the name `openLink` uses for each. The releases
 * link isn't here since it's opened from the navbar under a translated label. Only labels live here -
 * the actual URLs are decided in `crates/gui/src/links.rs`.
 */
export const APP_LINKS: Record<Exclude<Link, "releases">, string> = { repository: "Github", website: "vinicius.io" };

/**
 * The drawer's folded height. Lives here rather than in `Drawer` because the toaster also needs it,
 * to clear the strip when showing a notice.
 */
export const DRAWER_BLEEDING = 48;

/**
 * The drawer's unfolded body height (also how far a folded drawer is translated down). Beside
 * `DRAWER_BLEEDING` for the same cross-feature reason: the toaster and the canvas's pane labels
 * both need it.
 */
export const DRAWER_HEIGHT = 128;

/** The preview's zoom range: 1x fits the photo in its pane, 8x is the reference's ceiling. */
export const ZOOM_MIN = 1;
export const ZOOM_MAX = 8;

/** How far one wheel notch moves the zoom - a fixed step per event, matching the reference (design.md D5). */
export const ZOOM_WHEEL_STEP = 0.05;

/** How far the minus/plus buttons move the zoom, as the reference. */
export const ZOOM_BUTTON_STEP = 0.5;

/**
 * Longest edge requested for a thumbnail (see design.md D4 for the sizing math). 384 covers a 104px
 * cover-cropped thumbnail at 2x density for both 3:2 and 16:9 photos. The sidebar miniature reuses
 * this same bound so it can share the cached rendition instead of triggering a second render.
 * Fixed rather than `devicePixelRatio`-based so moving windows across monitors doesn't cause
 * re-fetching and duplicate cached renditions.
 */
export const THUMBNAIL_BOUND = 384;

/**
 * Size (px) of the three-dot menu glyph in the drawer's name bar. Named because the thumbnail menu's
 * `alignOffset` depends on it matching the trigger button's box exactly. The `<Ellipsis>` icon itself
 * doesn't read this constant (it's sized via Tailwind's `size-[15px]`); only `FileMenuTrigger`'s
 * button box needs to match.
 */
export const FILE_MENU_GLYPH = 15;

/** Gap (px) between the file options menu and the control that opened it, per the mockup. */
export const FILE_MENU_GAP = 16;

/**
 * The sidebar's padding. Named because the Add enhancement menu is anchored relative to the panel
 * edge (this + gap), not the button's own box.
 */
export const SIDEBAR_PADDING = 24;

/** Gap between the add-enhancement menu and the sidebar's leading edge. */
export const ADD_MENU_GAP = 16;

/**
 * Height (px) of one enhancement row in the sidebar. Named because the options panel is anchored at
 * half this value below the row's top edge.
 */
export const ENHANCEMENT_ROW_HEIGHT = 52;

/**
 * Longest edge requested for the Crop/Rotate dialog (sizing math like {@link THUMBNAIL_BOUND}).
 * Covers the cropper pane at 2x density on wide windows. Unlike the reference, which requests the
 * full-resolution original (costly to decode/transfer), Rust never upscales - so for images under
 * this bound the widget-to-photo coordinate factor is exactly 1.
 */
export const CROP_DIALOG_BOUND = 3072;

/** Minimum crop rectangle edge size, in widget pixels (reference's value). */
export const MIN_CROP_SIZE = 16;

/**
 * How much one wheel notch magnifies the photo inside the Crop/Rotate dialog. A *ratio* of current
 * scale (unlike the canvas's {@link ZOOM_WHEEL_STEP}, a fixed increment) since it magnifies the
 * dialog's transient view rather than a persisted per-image zoom.
 */
export const CROP_ZOOM_WHEEL_RATIO = 0.025;

/**
 * Longest edge requested for the Select faces dialog (sizing math like {@link THUMBNAIL_BOUND}).
 * Deliberately soft on very tall displays since this view is for identifying faces, not judging
 * restoration detail - and face boxes are percentages, unaffected by resolution.
 */
export const FACES_DIALOG_BOUND = 2048;
