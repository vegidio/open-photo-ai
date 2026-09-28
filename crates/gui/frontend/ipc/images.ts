import { convertFileSrc } from "@tauri-apps/api/core";
import { type CropInfo, cropQuery } from "./crop";
import { call } from "./invoke";
import { once } from "./once";

/**
 * Names of the Rust commands backing this module.
 *
 * Each must match a `#[tauri::command]` function name in `crates/gui/src/images/files.rs`. Renaming one
 * side alone is not a type error - `images.test.ts` pins this side, `generate_handler![]` pins the other.
 */
const OPEN_IMAGES_COMMAND = "open_images";
const DESCRIBE_IMAGES_COMMAND = "describe_images";
const INPUT_EXTENSIONS_COMMAND = "input_extensions";
const REVEAL_IMAGE_COMMAND = "reveal_image";

/**
 * URI scheme used to serve an opened image's pixels.
 *
 * Must match `SCHEME` in `crates/gui/src/images/serve.rs` and `img-src` in `tauri.conf.json`.
 */
const SCHEME = "opai";

/**
 * One image file the user opened, described without decoding it.
 *
 * Optional fields mean "could not be determined", not zero - a broken or missing file is still reported
 * rather than dropped from the batch. `width`/`height` are set together or not at all. `identity` is
 * absent only when the file couldn't be read at all, making it non-addressable (see {@link renditionUrl}).
 */
export type ImageRecord = {
    /** File path, for display only - never sent back through this. */
    path: string;
    /** XXH3-64 hash of the file's bytes, lowercase hex. Key for renditions, transforms, crop, export. */
    identity?: string;
    /** Width in pixels. */
    width?: number;
    /** Height in pixels. */
    height?: number;
    /** File extension, lowercase, no dot. Empty if none. */
    extension: string;
    /** File size in bytes. */
    size?: number;
};

/**
 * Why one of the image commands rejected.
 *
 * `kind` names the command that failed; `message` is an untranslated diagnostic. The first two kinds
 * mean the same failure (files couldn't be described, e.g. app closing) for different commands.
 *
 * **A dismissed or unopenable picker is not an error** - both resolve with an empty array, since the
 * platform reports them identically. An empty array means "no images added", never "something failed".
 */
export type ImagesError = {
    kind: "openImages" | "describeImages" | "revealImage";
    message: string;
};

/**
 * Opens the platform's file picker for images.
 *
 * `title`/`filterName` are pre-translated by this side; Rust holds no strings of its own. Resolves with
 * one record per chosen file, ordered by path. An empty array means no images were added (see
 * {@link ImagesError}), not a failure.
 *
 * Uses a custom command instead of `@tauri-apps/plugin-dialog` (not installed) because it also builds
 * the filter from the decoder's own format list and admits the files so their pixels become reachable.
 */
export const openImages = (title: string, filterName: string) =>
    call<ImageRecord[]>(OPEN_IMAGES_COMMAND, { title, filterName });

/**
 * Describes image files that arrived some other way (drag-and-drop, command line).
 *
 * Records are identical in shape to {@link openImages}'s. Nothing is filtered - a non-image file comes
 * back with no dimensions, same as a truncated one.
 */
export const describeImages = (paths: string[]) => call<ImageRecord[]>(DESCRIBE_IMAGES_COMMAND, { paths });

/**
 * Extensions the application opens, lowercase and without dots.
 *
 * Mirrors what the decoder (and picker filter) actually supports. Used by the drag-and-drop route to
 * reject unsupported files client-side, with a properly pluralized message.
 *
 * Fetched once and cached via {@link once}, since the list is compiled into the binary.
 */
const fetched = once(() => call<string[]>(INPUT_EXTENSIONS_COMMAND));

export const inputExtensions = () => fetched.get();

/** Forgets the fetched extensions. For tests only - see {@link once}. */
export const forgetInputExtensions = () => fetched.forget();

/**
 * Reveals an opened image in the platform's file manager, selected.
 *
 * Takes the record's own `path`; Rust refuses any path it never admitted via `openImages`/`describeImages`.
 *
 * Addressed by path rather than identity, since a file with no identity (unreadable) can still be a
 * valid target here - unlike {@link renditionUrl}.
 *
 * Uses a custom command instead of `@tauri-apps/plugin-opener` because that plugin's async command runs
 * on a Tauri worker thread, where all three platform implementations break (see `crates/gui/src/reveal.rs`).
 *
 * Rejections propagate rather than being swallowed - a moved/deleted file should surface as an error.
 */
export const revealImage = (path: string) => call<void>(REVEAL_IMAGE_COMMAND, { path });

/**
 * Builds the URL to draw an opened image from, bounded on its longest edge, optionally cropped.
 *
 * `<img src={renditionUrl(file.identity, 100)} />` is all that's needed for a thumbnail - Rust decodes,
 * bounds, encodes and streams the pixels over this app's own URI scheme, so formats the webview can't
 * decode (RAW, TIFF, HEIF) still render.
 *
 * `bound` caps the longest edge, preserving aspect ratio, and never enlarges. Omitted or 0 serves the
 * image at native size.
 *
 * `crop` frames the image before `bound` is applied, so the bound caps the framing's longest edge, not
 * the source image's. Omitted, the URL is unchanged from before this parameter existed.
 *
 * # Platform-specific URLs
 * Built via `convertFileSrc`, never by hand - the scheme differs between macOS/Linux (`opai://...`) and
 * Windows (`http://opai.localhost/...`).
 *
 * # Caching
 * The response is served immutable: identity (whole-file hash) plus bound plus crop are all in the URL,
 * so a given URL can never mean different pixels. The actual cache lives in Rust (`Renditions` in
 * `crates/gui/src/images/renditions.rs`), keyed the same way - not here, to avoid the Blob/object-URL
 * lifecycle management (revoke, `dispose`, in-flight map) that approach requires. Note that WebView2 is
 * the only backend that honors immutability at the HTTP layer; macOS/Linux rely entirely on the Rust-side cache.
 */
export const renditionUrl = (identity: string, bound = 0, crop?: CropInfo) => {
    const url = convertFileSrc(identity, SCHEME);
    const query = [...(bound > 0 ? [`size=${bound}`] : []), ...(crop ? [`crop=${cropQuery(crop)}`] : [])];

    return query.length > 0 ? `${url}?${query.join("&")}` : url;
};

/**
 * Like {@link renditionUrl}, but for a record that may have no `identity` - returns `undefined` in that
 * case rather than a URL that would 404. Centralizes the `file?.identity ? renditionUrl(...) : undefined`
 * pattern used across the canvas, preload and sidebar.
 */
export const renditionFor = (file: { identity?: string } | undefined, bound = 0, crop?: CropInfo) =>
    file?.identity ? renditionUrl(file.identity, bound, crop) : undefined;
