import { convertFileSrc } from "@tauri-apps/api/core";
import { type CropInfo, cropQuery } from "./crop";
import { call } from "./invoke";
import { once } from "./once";

/**
 * The names of the Rust commands, written out once on this side of the boundary.
 *
 * Each is written a second time in `crates/gui/src/images/files.rs`, as the name of a function carrying
 * `#[tauri::command]`, and nothing in either toolchain notices when one of the two is renamed alone:
 * a rename here is not a type error, it is an `invoke` that rejects at runtime. `images.test.ts` pins
 * this half; the Rust half is pinned by `generate_handler![]` refusing to compile against a function
 * that does not exist.
 */
const OPEN_IMAGES_COMMAND = "open_images";
const DESCRIBE_IMAGES_COMMAND = "describe_images";
const INPUT_EXTENSIONS_COMMAND = "input_extensions";
const REVEAL_IMAGE_COMMAND = "reveal_image";

/**
 * The URI scheme an opened image's pixels are served over.
 *
 * Written a third time in `crates/gui/src/images/serve.rs` as `SCHEME` and in `tauri.conf.json` as part of
 * `img-src`. A Rust test asserts those two agree; this one is pinned by `images.test.ts`, because a
 * rename that missed this file would be a window of broken images with nothing failing to compile.
 */
const SCHEME = "opai";

/**
 * One image file the user opened, described without decoding it.
 *
 * The optional properties are "the application could not tell", not zero. A file whose header cannot
 * be read, or that has gone missing since the user named it, is still reported - a selection is a
 * batch they assembled, and dropping the whole of it because one file is broken costs them the rest
 * for no gain.
 *
 * `width` and `height` come from one read of the header, so they are either both there or both
 * absent. `identity` is absent only when the file's bytes could not be read at all, and such a file
 * is never addressable: {@link renditionUrl} has nothing to build a URL from, which is exactly right,
 * because the application could not read it either.
 */
export type ImageRecord = {
    /** Where the file is, for display. Nothing is ever sent back through this. */
    path: string;
    /**
     * The XXH3-64 of the whole of the file's bytes, lowercase hexadecimal.
     *
     * The key everything about this photograph hangs off: the rendition URL, and later the per-image
     * transform, crop and export row. It describes bytes, so it cannot go stale the way a path can.
     */
    identity?: string;
    /** The photograph's width in pixels. */
    width?: number;
    /** The photograph's height in pixels. */
    height?: number;
    /** The file's extension, lowercase and without its dot. Empty where the name has none. */
    extension: string;
    /** The file's size on disk, in bytes. */
    size?: number;
};

/**
 * Why one of the image commands rejected.
 *
 * `kind` is the `#[serde(tag = "kind")]` on Rust's `ImagesError`, spelled as the command it belongs
 * to, so a caller reading it is reading what it asked for. `message` is the reason in full, in
 * English and untranslated, for the same reason `LogsError`'s is: it is a diagnostic to be copied
 * into a bug report.
 *
 * The first two mean the same thing - the chosen files could not be described, the application having
 * closed while it happened - and differ only in which command was asked. In practice a caller shows
 * the same message for either. `revealImage` is the third and is a different failure: the file
 * manager would not open, or the file named was not one this application opened.
 *
 * **Neither a dismissal nor a picker that failed to open is one of these.** Both resolve with no
 * files: choosing nothing is a choice, and the platform's picker reports a failure to open as the
 * same empty answer, so Rust does not claim to tell the two apart. An empty array is therefore "no
 * images were added", never "something went wrong".
 */
export type ImagesError = {
    kind: "openImages" | "describeImages" | "revealImage";
    message: string;
};

/**
 * Ask the user for image files through the platform's own picker.
 *
 * `title` and `filterName` are passed in already translated, because this side owns the application's
 * catalogue. The Rust side deliberately holds no strings of its own: a second catalogue there would
 * be a second thing to keep in step with thirteen locales. The extension list is the other half of
 * that split and stays in Rust, derived from what the decoder actually opens.
 *
 * Resolves with one record per chosen file, ordered by path. **An empty array is that no images were
 * added** - a dismissal, or a picker the platform would not open, which it reports as the same
 * answer - and not a failure. See {@link ImagesError}.
 *
 * Calls a command of this application's own rather than `@tauri-apps/plugin-dialog`, which is not
 * installed. The command has work the plugin's cannot do: it builds the filter from the core
 * library's own list of decodable formats, admits what was chosen so its pixels become reachable, and
 * hands back records rather than bare paths.
 */
export const openImages = (title: string, filterName: string) =>
    call<ImageRecord[]>(OPEN_IMAGES_COMMAND, { title, filterName });

/**
 * Describe image files that arrived some other way - a drag onto the window, or later a path on the
 * command line.
 *
 * The records are indistinguishable from {@link openImages}' for the same files, and deliberately so:
 * a file dragged onto the window is the same photograph as a file chosen from a dialog. Ordered by
 * path, the same way.
 *
 * Nothing is filtered on the way in. A path that is not an image comes back described as far as it
 * could be, with no dimensions, which is the same answer a truncated photograph gets.
 */
export const describeImages = (paths: string[]) => call<ImageRecord[]>(DESCRIBE_IMAGES_COMMAND, { paths });

/**
 * The extensions the application opens, lowercase and without their dots.
 *
 * Without the dots is what makes a comparison against a dropped path's own extension a string
 * equality rather than a normalisation. The list is what the decoder actually opens - the same list
 * the picker is filtered to - so a format the library gains is a format the window accepts without
 * this file being touched, and one it loses is a format the window stops claiming.
 *
 * The one caller is the drag-and-drop route, which is the only way a file reaches this application
 * without having passed the picker's filter. What that route refuses is named to the user here rather
 * than in Rust, because the notice needs a plural form only the catalogue gets right.
 *
 * **Fetched once and kept**, through the same {@link once} `ipc/catalogue.ts` uses and for a stronger
 * reason: the list is compiled into the binary, so nothing can change it while the process runs.
 */
const fetched = once(() => call<string[]>(INPUT_EXTENSIONS_COMMAND));

export const inputExtensions = () => fetched.get();

/** Forgets the fetched extensions. For tests alone - see {@link once}. */
export const forgetInputExtensions = () => fetched.forget();

/**
 * Show an opened image to the user in the platform's own file manager, with the file selected.
 *
 * Takes the record's own `path` - the one Rust handed back - and **the Rust side refuses one it never
 * admitted**. That is the same entitlement the `opai://` scheme establishes, applied to the other
 * door out of the window: what the user opened is the whole of what the interface may name. The
 * reference implementation's `RevealInFileManager` accepts any path the webview sends; this does not.
 *
 * By path rather than by identity, which is what {@link renditionUrl} uses. An identity is absent
 * exactly when the file's bytes could not be read, and such a file is still an open image - the one a
 * user is most likely to want to look at on disk. Addressing this by identity would have withheld it
 * from precisely that case.
 *
 * Calls a command of this application's own rather than `@tauri-apps/plugin-opener`, for the reason
 * `ipc/logs.ts` states at length: the plugin's own command is `async`, so Tauri runs it on a runtime
 * worker, and all three of its platform implementations break there. `crates/gui/src/reveal.rs` holds
 * the thread discipline and the references.
 *
 * **The rejection is propagated rather than swallowed.** A file can be moved or deleted after it was
 * opened, and a menu item that quietly does nothing reads as a broken one.
 */
export const revealImage = (path: string) => call<void>(REVEAL_IMAGE_COMMAND, { path });

/**
 * Where to draw an opened image from, at a bound on its longest edge and under a framing.
 *
 * `<img src={renditionUrl(file.identity, 100)} />` is the whole of how a thumbnail is drawn. The
 * pixels are decoded, bounded and encoded by Rust and streamed over the application's own URI scheme,
 * so RAW, TIFF and HEIF arrive as something the webview can actually draw - none of which it could
 * decode itself.
 *
 * `bound` caps the longest edge and preserves the aspect ratio. **It never enlarges**: a bound above
 * the photograph's own longest edge serves it unchanged. Omitting it, or passing 0, serves the
 * photograph at its own dimensions.
 *
 * `crop` frames the photograph before the bound is applied, so the bound caps the longest edge of the
 * *framing* rather than of the photograph it was taken from. Omitting it serves the whole photograph,
 * and the URL is then byte-for-byte the one this function has always built - which is what keeps
 * every rendition, every cached response and every test of them untouched by this argument existing.
 *
 * A bounded framing is cropped here where the reference's is not: `GetImage` applies a crop only when
 * `size == 0`, so the old application's sidebar miniature shows the *uncropped* photograph beside a
 * cropped canvas. That is an artefact of cropping at full resolution being too expensive for a
 * thumbnail, and Rust removes it by reducing the photograph before it turns it.
 *
 * # Built by `convertFileSrc`, never by hand
 *
 * The URL differs by platform - `opai://localhost/...` on macOS and Linux, `http://opai.localhost/...`
 * on Windows - and this is the one place that difference is allowed to exist. Writing either form out
 * would be a build that works on the machine it was developed on.
 *
 * # The response may be kept, and there is still no cache here
 *
 * Rust serves it immutable, which is safe rather than hopeful: the identity covers every byte of the
 * file and the bound is in the URL, so a given URL cannot come to mean different pixels.
 *
 * The framing joins the identity and the bound in that promise rather than weakening it: Rust keys
 * what it has already produced on the whole request, so two framings of one photograph are two
 * entries and neither can be served where the other was asked for.
 *
 * **Only WebView2 acts on that.** A custom scheme on macOS and Linux is served by a handler the
 * webview's resource cache does not sit in front of, so the directive is true, correct and ignored -
 * which showed up as a drawer that re-decoded a photograph every time a thumbnail scrolled out of the
 * virtualized strip and back in, about 230ms of work for pixels produced seconds earlier.
 *
 * The cache that fixes it is in Rust, keyed by the same identity, bound and crop - see `Renditions` in
 * `crates/gui/src/images/renditions.rs`. It is deliberately not here: a cache on this side would hold Blobs
 * behind object URLs, which brings back the revoke discipline, the `dispose` hook and the in-flight
 * map the reference spends 150 lines of `utils/image.ts` on. Rust holds encoded bytes instead, an
 * order of magnitude smaller, shared by every `<img>` that asks, and outliving a reload of the window.
 */
export const renditionUrl = (identity: string, bound = 0, crop?: CropInfo) => {
    const url = convertFileSrc(identity, SCHEME);
    const query = [...(bound > 0 ? [`size=${bound}`] : []), ...(crop ? [`crop=${cropQuery(crop)}`] : [])];

    return query.length > 0 ? `${url}?${query.join("&")}` : url;
};

/**
 * Where to draw an opened *record* from, or `undefined` where nothing can serve its pixels.
 *
 * The rule {@link renditionUrl} implies, stated once: a record with no `identity` was never admitted,
 * so there is no rendition to ask for and the caller is given nothing rather than a URL that would
 * 404. Every region that draws a photograph holds a record rather than a bare identity, which is why
 * this rather than the three copies of `file?.identity ? renditionUrl(file.identity) : undefined`
 * that the canvas, the preload and the sidebar each used to carry.
 *
 * An `<img>` handed `undefined` draws as broken, which is what a file deleted between being described
 * and being drawn does anyway - the case is reported by the record, not invented here.
 */
export const renditionFor = (file: { identity?: string } | undefined, bound = 0, crop?: CropInfo) =>
    file?.identity ? renditionUrl(file.identity, bound, crop) : undefined;
