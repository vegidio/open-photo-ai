// **Its one writer is the Crop/Rotate dialog**, through `features/crop/useCropController.ts`.
/**
 * How one photograph is framed: two flips, a turn, and the rectangle to cut.
 *
 * The frontend's half of Rust's `Crop`, which `crates/gui/src/images/crop.rs` documents at length -
 * including why the turn is an integer count of thousandths of a degree rather than a number of
 * degrees.
 */
export type CropInfo = {
    /** The rectangle's left edge, in the *rotated* photograph's coordinate space. */
    left: number;
    /** The rectangle's top edge, in the rotated photograph's coordinate space. */
    top: number;
    /** The rectangle's width. Never zero - Rust refuses a rectangle with no area outright. */
    width: number;
    /** The rectangle's height. Never zero, for the same reason. */
    height: number;
    /** The turn, in thousandths of a degree, positive clockwise. */
    millidegrees: number;
    /** Whether the photograph is mirrored left-to-right, before anything else. */
    flipHorizontal: boolean;
    /** Whether the photograph is mirrored top-to-bottom, after the horizontal flip. */
    flipVertical: boolean;
};

// Four spellings rather than two booleans, because the whole crop is one positional parameter and a
// pair of `true`/`false` in the middle of six numbers reads worse and parses no better.
/** The two flips as the URL spells them: `-`, `h`, `v` or `hv`. */
const flips = (crop: CropInfo) =>
    crop.flipHorizontal ? (crop.flipVertical ? "hv" : "h") : crop.flipVertical ? "v" : "-";

// Written a second time in `crates/gui/src/images/serve.rs` as `crop_of`, which refuses anything this
// does not produce and documents why the grammar is that strict and positional rather than JSON.
// `crop.test.ts` pins this half against it. The `enhance` command takes the same value as JSON - see
// Rust's `Crop` for why one value has two spellings.
/**
 * A framing spelled for the `crop` parameter of a rendition URL.
 *
 * `<left>,<top>,<width>,<height>,<millidegrees>,<flips>` - six positional fields, no percent-encoding
 * and nothing optional.
 */
export const cropQuery = (crop: CropInfo) =>
    `${crop.left},${crop.top},${crop.width},${crop.height},${crop.millidegrees},${flips(crop)}`;

/**
 * A framing as a value to compare, with no framing at all as the empty string.
 *
 * Two framings are the same framing when every field agrees, which {@link cropQuery} already spells
 * in full; the empty string is one it never produces. Compared by key rather than by reference, so
 * "found at this framing" does not rest on how the crop store happens to hand its values out - a
 * framing written again with the same fields is the same framing.
 */
export const cropKey = (crop: CropInfo | undefined) => (crop ? cropQuery(crop) : "");

/**
 * The dimensions of a photograph as it is framed, or the file's own where it is drawn whole.
 *
 * **Read off the crop rather than off the pixels**, which is what makes it answerable before they
 * arrive.
 *
 * Both properties are absent for a file whose header could not be read and which carries no framing:
 * the application could not measure it, and zero would be a measurement. A framing always has both,
 * because a rectangle with no area is not a framing Rust will describe.
 */
export const framedDimensions = (
    file: { width?: number; height?: number } | undefined,
    crop: CropInfo | undefined,
): { width?: number; height?: number } => {
    // The canvas lays out the `<img>`'s box from this and only then loads the rendition - see
    // `hooks/useSettledFile.ts` - so a size taken from the decoded picture would be a size arriving a
    // decode too late. It is correct because Rust serves an *unbounded* framing at exactly the
    // rectangle's dimensions, which is the property `reduced` in crates/gui/src/images/serve.rs keeps
    // for precisely this reason. The reference computes the same thing in its `cropDimensions`.
    if (crop) return { width: crop.width, height: crop.height };

    return {
        ...(file?.width !== undefined && { width: file.width }),
        ...(file?.height !== undefined && { height: file.height }),
    };
};
