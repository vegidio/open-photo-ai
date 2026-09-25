import type { Link } from "@/ipc/links";

/**
 * The application's name.
 *
 * Deliberately not an i18n key, for the reason the Wails app gives for the same constant: a brand
 * name must read identically in every language, and carrying it in thirteen catalogues only creates
 * thirteen chances for one of them to translate it.
 */
export const APP_NAME = "Open Photo AI";

/**
 * The copyright line the About dialog draws, as the reference writes it, em dash included.
 *
 * Not an i18n key, for the reason {@link APP_NAME} gives: a name and a notice read the same in every
 * language.
 */
export const APP_COPYRIGHT = "© 2025—2026, Vinicius Egidio";

/**
 * What the About dialog's two links are labelled, keyed by the name `openLink` sends for each. The
 * releases page is not one of them: the navbar's "Update Available" opens it, under a translated label.
 *
 * Only the labels: the address each one opens is decided in `crates/gui/src/links.rs`, so the window
 * never holds one. Untranslated like {@link APP_COPYRIGHT}, as in the reference.
 */
export const APP_LINKS: Record<Exclude<Link, "releases">, string> = { repository: "Github", website: "vinicius.io" };

/**
 * The drawer's folded height: the header that stays flush with the window's bottom edge whether the
 * drawer is folded or not.
 *
 * Here rather than in `Drawer` because it is not only the drawer's own business - the toaster clears
 * it, so the notice lands above the strip rather than behind it - and a second component that had to
 * know this number could only learn it by importing a feature or by copying the 48.
 */
export const DRAWER_BLEEDING = 48;

/**
 * The drawer's body: the strip of thumbnails an unfolded drawer shows, and exactly the distance a
 * folded one is translated down by.
 *
 * Beside `DRAWER_BLEEDING` rather than in `Drawer`, now that it has the same second reader for the
 * same reason: the toaster clears the unfolded body as well as the folded header, and the canvas's
 * pane labels ride above whichever of the two is showing. A component that had to know this number
 * could otherwise only learn it by importing a feature or by copying the 128.
 */
export const DRAWER_HEIGHT = 128;

/**
 * The range the preview's zoom is kept between: 1x is the photograph drawn whole in the pane it is
 * in, and 8x is the reference's own ceiling.
 *
 * Here rather than in `DrawerHeader`, which held them while the slider was the only thing that knew
 * them. The readers now are the slider and its two step buttons, the canvas's wheel handler and the
 * store that clamps every write - four of them across three features, which is what moved these up.
 */
export const ZOOM_MIN = 1;
export const ZOOM_MAX = 8;

/**
 * How far one wheel notch moves the scale.
 *
 * The reference's `ZOOM_WHEEL_STEP`, and it is a fixed step per *event* rather than a function of
 * the event's magnitude: a hard flick of a trackpad moves the scale exactly as far per event as a
 * slow one. Parity, deliberately - see design.md D5.
 */
export const ZOOM_WHEEL_STEP = 0.05;

/** How far the minus and plus buttons either side of the slider move the scale, as the reference. */
export const ZOOM_BUTTON_STEP = 0.5;

/**
 * The longest edge a thumbnail asks the protocol for.
 *
 * The arithmetic (D4): the strip's body is 128px with 12px of vertical padding, so an item is a
 * 104px square, and the design fills it *cropped* - `object-cover`. A cover crop is driven by the
 * **short** edge, so on a 2x display the short edge must reach 208 device pixels; bounding the long
 * edge to reach that takes 312 for a 3:2 photograph and 370 for 16:9. 384 clears both.
 *
 * The sidebar's miniature asks for this same bound, at ~144px drawn - comfortably under it. That is
 * deliberate rather than convenient: sharing the bound means sharing the URL, so the miniature is
 * served from a rendition the strip has already had Rust produce and costs no second render.
 *
 * One constant for every thumbnail on every machine, rather than a bound derived from
 * `devicePixelRatio`: the bound is in the URL, so a bound that moved with the display would
 * re-request every thumbnail when the window was dragged to another monitor, and leave the webview's
 * cache holding two renditions of every photograph.
 *
 * The reference asks for 100, which was chosen when a thumbnail was base64 over a Wails IPC channel
 * and every byte was paid twice. It is below the item's own CSS size before density is considered at
 * all, so following it would ship a visibly soft strip on every Retina display.
 */
export const THUMBNAIL_BOUND = 384;

/**
 * The three-dot glyph that opens the file options menu, in pixels.
 *
 * The design's own size for it in the drawer's name bar. It is a named constant because two unrelated
 * things have to agree on it: the **trigger button's box** at each of the two call sites, and the
 * thumbnail menu's `alignOffset` - which places the menu's leading edge at the control's trailing
 * edge by shifting it the control's own width, and is therefore correct only while that box *is* the
 * glyph's box.
 *
 * A later change giving that trigger a comfortable hit area would move the menu sideways by whatever
 * padding it added, and nothing would fail - a rendered-geometry assertion is not available under
 * jsdom. One constant read by both is the whole of the defence, which is why the offset is written in
 * terms of this rather than as the number it comes to.
 *
 * **The `<Ellipsis>` inside the trigger does not read it**, and cannot: it is sized by Tailwind's
 * `size-[15px]`, which is a class name rather than a value, and every other icon in this codebase is
 * sized the same way. That literal is cosmetic - the icon fills the button, and it is the *button*
 * the menu is measured against, so an icon that drifted from this number would look wrong rather than
 * anchor wrong. There is one of it to change, in `FileMenuTrigger`, which is the single control both
 * the navbar and the drawer thumbnail draw.
 */
export const FILE_MENU_GLYPH = 15;

/**
 * How far the file options menu clears the control it opened from, in pixels.
 *
 * 16px, which is what the mockup draws: it places the menu at `(+16, +16)` from the control's corner,
 * consistent in both axes and so intentional rather than eyeballed. Shared by both anchorings, and by
 * both axes of the thumbnail's - so the corners do not meet and the menu clears the glyph diagonally.
 */
export const FILE_MENU_GAP = 16;

/**
 * The sidebar's own padding, which the Add enhancement button sits inside.
 *
 * A named constant because the menu that button opens is anchored 16px clear of the *panel's* edge
 * while Radix measures from the *button's* box - so the offset is this plus the gap, and a change to
 * the panel's padding that left the offset alone would slide the menu sideways.
 */
export const SIDEBAR_PADDING = 24;

/** How far the add-enhancement menu clears the sidebar's leading edge, as the design draws it. */
export const ADD_MENU_GAP = 16;

/**
 * The height of one enhancement row in the sidebar, in pixels.
 *
 * A named constant because the options panel is anchored against it: the design puts the panel's top
 * edge at the row's vertical centre (screen 11), which is half of this walked down from the row's
 * top. The row itself is sized by Tailwind's `min-h-13`, which is a class name rather than a value -
 * the same split {@link FILE_MENU_GLYPH} documents, and the same consequence: a row given a
 * different height would move the panel rather than fail.
 */
export const ENHANCEMENT_ROW_HEIGHT = 52;

/**
 * The longest edge the Crop/Rotate dialog asks the protocol for.
 *
 * The arithmetic, derived the way {@link THUMBNAIL_BOUND} is derived rather than picked: the cropper
 * pane is the window less the dialog's own 32px inset on both sides, its 32px of padding on both
 * sides, and the 256px settings column - so on a 1920-wide window it is about 1550 CSS pixels across.
 * 3072 covers that at 2x density with room for a wider window.
 *
 * One fixed constant rather than a bound derived from `devicePixelRatio`, for the reason
 * {@link THUMBNAIL_BOUND} gives: the bound is in the URL, so a bound that moved with the display
 * would re-request the photograph when the window was dragged to another monitor.
 *
 * The reference asks for the *uncropped original at full resolution* instead. A 9504x6336 scan is
 * then a full decode, a JPEG encode, a transfer and a second decode - several seconds and hundreds of
 * megabytes of decoded pixels in the webview - every time the dialog opens, to fill a widget a fifth
 * of that size. What bounding costs is that the dialog's coordinates are no longer the photograph's:
 * one factor converts at exactly one boundary, and `features/crop/useCropController.ts` documents
 * where. Rust never enlarges, so a photograph under this bound is served unchanged and that factor is
 * exactly 1 - which is the common case, and is exact.
 */
export const CROP_DIALOG_BOUND = 3072;

/**
 * The smallest a framing's rectangle may be on either edge, in the *widget's* own pixels.
 *
 * The reference's value. Applied in the widget's space rather than in the photograph's, which is the
 * simple spelling and also the conservative one: the factor above is never below 1, so 16 reduced
 * pixels is at least 16 source pixels.
 */
export const MIN_CROP_SIZE = 16;

/**
 * How far one wheel notch magnifies the photograph inside the Crop/Rotate dialog.
 *
 * The reference's `CROP_ZOOM_WHEEL_RATIO`, passed to the widget rather than applied here - it is a
 * *ratio* of the current scale, where the canvas's own {@link ZOOM_WHEEL_STEP} is a fixed step added
 * to it. The two are separate numbers because they move different things: this magnifies what the
 * dialog draws and is never recorded in a framing, and that magnifies the photograph on the canvas
 * and is kept per image.
 */
export const CROP_ZOOM_WHEEL_RATIO = 0.025;

/**
 * The longest edge the Select faces dialog asks the protocol for.
 *
 * The arithmetic, derived the way {@link THUMBNAIL_BOUND} and {@link CROP_DIALOG_BOUND} are rather
 * than picked: the picture box is the window's height less the dialog's 64px of vertical inset, the
 * 48px title bar and the 52px footer - about 916 CSS pixels on a 1080-tall window - and the window's
 * width less 64. 2048 covers a ~1024 CSS-pixel picture at 2x density, which covers that window; a
 * much taller display draws it slightly soft.
 *
 * That trade is taken deliberately: this is a surface for telling faces apart and clicking them, not
 * for judging restoration detail. The alternative is what the reference does - a full-resolution
 * decode, JPEG encode, transfer and second decode of a 9504x6336 scan to fill a box a fifth of its
 * size. Nothing about the boxes depends on it either: they are percentages of the framed photograph,
 * so a softer picture is a softer picture and nothing else.
 *
 * One fixed constant rather than one derived from `devicePixelRatio`, for the reason
 * {@link THUMBNAIL_BOUND} gives: the bound is in the URL, so a bound that moved with the display
 * would re-request the photograph when the window was dragged to another monitor.
 */
export const FACES_DIALOG_BOUND = 2048;
