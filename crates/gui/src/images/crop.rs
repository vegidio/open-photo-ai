//! What a framing is, and what it does to a photograph's pixels and to its identity.
//!
//! A crop is the flip, the turn and the rectangle a user dragged in the Crop/Rotate dialog. It reaches
//! this application twice over — on the `opai://` URL a rendition is asked for with, and as an argument
//! to the `enhance` command — and [`frame`] is the one place either of them is applied: directly for the
//! URL's, and through [`cropped`] for the command's.

// Why this is in `gui` and not in `opai`: `opai` reads and writes image files and runs inference. A framing is none
// of the three: it exists because a window let someone drag one, in the coordinate space one widget defines, which is
// also what fixes the flip-flip-rotate-cut order below. Every caller is in this crate — `super::serve`, which answers
// what the window draws, `crate::enhance::run`, which prepares what a run is given, and `crate::faces`, which
// prepares what a detection is asked about — and the reference implementation draws the same line, with every one of
// its crop sites under `cmd/gui` and none in its core library.
//
// This is a plain function over an `opai::Picture`, so a second front end moves it into `opai` without rewriting it.

use std::borrow::Cow;
use std::path::PathBuf;

use image::DynamicImage;
use image::metadata::Orientation;
use opai::Picture;
use serde::Deserialize;

use super::files::Opened;

// A thousandth of a degree over a 60-megapixel edge is a fifth of a pixel, so nothing a user can express is lost to
// the unit; see `Crop` for why the turn is an integer.
/// How many of [`Crop::millidegrees`] make one degree.
const MILLIDEGREES_PER_DEGREE: f64 = 1_000.0;

/// The framing a user applied to one photograph: two flips, a turn, and the rectangle to cut.
///
/// The turn is an integer count of millidegrees, and the rectangle is in the *rotated* photograph's
/// coordinate space, which is what the widget reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
// Two spellings, one value: six positional fields on the `opai://` URL, parsed by `super::serve`, and the `crop`
// argument to the `enhance` command, deserialized through `Wire`. Two encodings is the cost of having two doors. The
// URL's is pinned by a test on both sides of the boundary — the same arrangement this application already makes for
// every command name and for the scheme — and the command argument's from the window's side alone, by the `ipc`
// tests that send it.
//
// Through [`Wire`] rather than derived straight onto the fields, so that a crop arriving from the window
// passes [`Crop::new`] exactly as one parsed out of a URL does — otherwise this type would have one door
// that enforces its invariant and one that does not.
#[serde(try_from = "Wire")]
pub(crate) struct Crop {
    // The reference's `CropInfo` fields, minus its "a zero value means no crop" convention — which Rust spells as
    // `Option<Crop>` instead, so a crop that describes nothing cannot be built at all.
    /// Whether the photograph is mirrored left-to-right, before anything else.
    flip_horizontal: bool,
    /// Whether the photograph is mirrored top-to-bottom, after the horizontal flip.
    flip_vertical: bool,
    // An integer count of millidegrees, not a float of degrees: a crop is half of two cache keys — `Renditions`' map
    // key and the identity a run's input is given — and `f64` is neither `Eq` nor `Hash`, because `NaN != NaN` and
    // `0.0 == -0.0` while their bit patterns differ. Fixing the unit at a thousandth of a degree makes this a plain
    // `Copy + Eq + Hash` value: the map key is exact, the derived identity is exact, and the URL carries an integer
    // rather than whatever `f64::to_string` produced on the frontend. `Crop::rotation_degrees` is what the rotation
    // is actually performed at.
    /// The turn, in thousandths of a degree, positive clockwise.
    millidegrees: i32,
    /// The rectangle's left edge, in the rotated photograph.
    left: u32,
    /// The rectangle's top edge, in the rotated photograph.
    top: u32,
    /// The rectangle's width. Never zero — see [`Crop::new`].
    width: u32,
    /// The rectangle's height. Never zero — see [`Crop::new`].
    height: u32,
}

// Written a second time in `frontend/ipc/crop.ts` as `CropInfo`, which `enhance.test.ts` pins.
//
// `deny_unknown_fields` for the reason the URL's parser refuses a seventh field: a crop this application does not
// understand must not be read as the part of it that it does.
/// The shape a crop crosses the IPC boundary in, on its way to becoming a [`Crop`]. Unknown fields are refused.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Wire {
    // Named fields rather than the URL's six positional ones, because a command argument is JSON the frontend
    // composes from a typed value and nothing has to be readable in a log line. `millidegrees` names its own unit,
    // which is what stops a float number of degrees being sent to a field that cannot hold one.
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    millidegrees: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
}

impl TryFrom<Wire> for Crop {
    type Error = &'static str;

    fn try_from(wire: Wire) -> Result<Self, Self::Error> {
        Crop::new(
            wire.left,
            wire.top,
            wire.width,
            wire.height,
            wire.millidegrees,
            wire.flip_horizontal,
            wire.flip_vertical,
        )
        .ok_or("a crop's rectangle must have a width and a height")
    }
}

impl Crop {
    /// A framing, or `None` where the rectangle has no area.
    ///
    /// Everything else is legal — a rectangle running past an edge is brought inside the picture when it is
    /// cut rather than refused here.
    pub(crate) fn new(
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        millidegrees: i32,
        flip_horizontal: bool,
        flip_vertical: bool,
    ) -> Option<Self> {
        // The one door into this type, which is why the fields are private: a zero-width or zero-height rectangle
        // frames nothing, and a type that can describe nothing is one every consumer has to check. Past the edge is
        // not refused because what it means depends on a photograph this type has never seen.
        if width == 0 || height == 0 {
            return None;
        }

        Some(Self { flip_horizontal, flip_vertical, millidegrees, left, top, width, height })
    }

    // No accessor for the flips, the turn or the rectangle's origin. Nothing outside this file reads
    // one: the composition below is in this module and takes the fields directly, and a caller that
    // wants to know whether two framings are the same asks `==`, which is what both cache keys do.
    // One is added when a caller needs it rather than in anticipation of one.

    /// The rectangle's width, which is the width of what a framing is served at.
    pub(crate) fn width(self) -> u32 {
        self.width
    }

    /// The rectangle's height, which is the height of what a framing is served at.
    pub(crate) fn height(self) -> u32 {
        self.height
    }

    /// The turn in degrees, which is what [`rust_sak::image::rotate`] takes.
    pub(crate) fn rotation_degrees(self) -> f64 {
        // The one place the integer becomes a float, and deliberately the only one: a caller that did this
        // arithmetic itself would be a second spelling of the unit.
        f64::from(self.millidegrees) / MILLIDEGREES_PER_DEGREE
    }

    /// The same framing, in a coordinate space `factor` times the size. The flips and the turn are untouched,
    /// and only the rectangle moves.
    ///
    /// The result is still a legal [`Crop`]: a rectangle scaled to nothing is floored at one pixel on
    /// each edge.
    pub(crate) fn scaled(self, factor: f64) -> Self {
        // What lets a bounded rendition reduce the photograph *first* and frame the small copy; see `serve::render`.
        // The flips and the turn mean nothing different at another size, and the one-pixel floor is the same floor
        // the cut itself applies.
        let scale = |value: u32| (f64::from(value) * factor).round().max(0.0) as u32;

        Self {
            left: scale(self.left),
            top: scale(self.top),
            width: scale(self.width).max(1),
            height: scale(self.height).max(1),
            ..self
        }
    }
}

// `opai`'s `identity_after` folds an operation's cache tag and its `identity_of_data` folds a discriminator of its
// own, and both are `pub(crate)` there — so a test on this side cannot assert the three can never collide the way
// `opai`'s own test asserts it for two. What makes it true is that no operation's cache tag emits this word, and that
// is stated here because a test cannot say it.
/// The word folded into a cropped picture's identity, which is what keeps this derivation clear of the
/// two `opai` makes internally.
const DISCRIMINATOR: &str = "crop";

/// Frames `picture` as `crop` describes, and names the pixels that produces. Blocking.
///
/// Flip, flip, turn, cut — in that order. A positive angle turns clockwise. The turn is skipped entirely
/// when there is none; otherwise the result is promoted to the alpha-bearing sibling of the input's colour
/// type, because a turn always exposes corners no source pixel covers.
///
/// Nothing fails. The origin is clamped into the picture and the extent to what remains, with a floor of
/// one pixel on each edge — so a rectangle running past an edge frames what exists, and one wholly outside
/// frames the nearest pixel that does.
///
/// A framing that changes nothing — no flip, no turn, and a rectangle covering the whole picture — answers
/// the source, identity and all. Otherwise the result is named by [`identity_of`], and its path is the
/// source's own.
pub(crate) fn cropped(picture: &Picture, crop: Crop) -> Picture {
    // Blocking, because its one caller already runs it on a blocking thread: `enhance` and `faces` both reach this
    // through [`load_framed`], which hands it to a `spawn_blocking`.
    match frame(picture.pixels(), crop) {
        // Borrowed is [`frame`]'s answer for a framing that changed nothing, so the source picture is what
        // this is — identity included, rather than the same pixels under a name of their own.
        //
        // This mirrors `opai::process`'s own "an empty chain returns the source unchanged, identity included", and
        // it is not a micro-optimisation: the Crop/Rotate dialog can be opened and dismissed having changed
        // nothing, and without this rule that would mint a new identity for identical pixels and re-run a chain
        // that had already been computed.
        Cow::Borrowed(_) => picture.clone(),
        // `opai::Picture::new` is documented as the single reviewed place a caller pairs pixels it computed itself
        // with the identity it derived for them, and this is that case. Folding through the same
        // `rust_sak::crypto::xxh3_string` every derived identity in this project is folded with is what makes a
        // framed picture indistinguishable from any other picture to the run cache `opai::process` keys on, and
        // what spares this the reference's two hand-synchronised token formats.
        //
        // The path is the source's own: these pixels came from that file, and `Picture`'s path is documented as
        // where the pixels came from rather than where anything is going.
        Cow::Owned(framed) => Picture::new(picture.path(), framed, identity_of(picture.identity(), crop)),
    }
}

/// The pixel half of [`cropped`]: the flip, the flip, the turn and the cut, with no identity derived.
///
/// `Cow::Borrowed` is the framing that changes nothing, which is how [`cropped`] tells that case apart
/// without asking a second time.
pub(super) fn frame(pixels: &DynamicImage, crop: Crop) -> Cow<'_, DynamicImage> {
    if changes_nothing(pixels, crop) {
        return Cow::Borrowed(pixels);
    }

    // The order is fixed rather than chosen per request, because the rectangle is expressed in the *rotated*
    // picture's coordinate space: it is what the cropper widget measured against, so it only means anything once
    // the same two flips and the same turn have been applied. Positive clockwise is the direction
    // `rust_sak::image::rotate` documents and the direction an on-screen control reports. The reference negates its
    // angle here; that is `imaging.Rotate` being counter-clockwise, not a difference in what the widget said.
    //
    // `DynamicImage`'s own flips and cut rather than `image::imageops`' free functions, which are the same
    // operations reached through `GenericImageView` — and would therefore see a `DynamicImage` as
    // `Rgba<u8>` and flatten a developed sixteen-bit RAW on the way through. These dispatch on the
    // variant and keep it.
    //
    // With no turn, the rectangle is cut first and only the cut is flipped. A flip maps the picture onto itself, so
    // the rectangle measured in the flipped picture is its mirror image in the source — and copying out that mirror
    // and flipping it is the same pixels as flipping the whole photograph and copying out the rectangle, for the cost
    // of the rectangle rather than a second full-size buffer. Pinned against the flip-first composition by
    // `cutting_before_flipping_is_the_same_pixels`.
    if crop.millidegrees == 0 {
        // Against `pixels` rather than a flipped copy of it: a flip keeps the dimensions, so the clamp is the same.
        let (left, top, width, height) = inside(pixels, crop);

        // `inside` keeps `left + width` within the width and `top + height` within the height, so neither wraps.
        let left = if crop.flip_horizontal { pixels.width() - left - width } else { left };
        let top = if crop.flip_vertical { pixels.height() - top - height } else { top };

        let mut cut = pixels.crop_imm(left, top, width, height);

        if crop.flip_horizontal {
            cut.apply_orientation(Orientation::FlipHorizontal);
        }

        if crop.flip_vertical {
            cut.apply_orientation(Orientation::FlipVertical);
        }

        return Cow::Owned(cut);
    }

    // A turn is about the picture's centre, so the flips cannot be moved past it this cheaply: they are applied to the
    // whole photograph, then the turn, then the cut.
    let mut framed = Cow::Borrowed(pixels);

    if crop.flip_horizontal {
        framed.to_mut().apply_orientation(Orientation::FlipHorizontal);
    }

    if crop.flip_vertical {
        framed.to_mut().apply_orientation(Orientation::FlipVertical);
    }

    // Only reached with a turn. The early return above is not only the cheaper path: `rotate` promotes its result to
    // the alpha-bearing sibling of the input's colour type, and a plain rectangular cut has no exposed corners to be
    // honest about. Turning by zero would make every framing a PNG.
    framed = Cow::Owned(rust_sak::image::rotate(&framed, crop.rotation_degrees()));

    let (left, top, width, height) = inside(&framed, crop);

    Cow::Owned(framed.crop_imm(left, top, width, height))
}

/// Whether `crop` frames the whole of `pixels` exactly as they are.
///
/// A rectangle *larger* than the picture counts, because the cut clamps it back to the picture's own
/// bounds.
fn changes_nothing(pixels: &DynamicImage, crop: Crop) -> bool {
    // It selects the same pixels, and answering anything but the source for them would be minting a second identity
    // for one photograph.
    !crop.flip_horizontal
        && !crop.flip_vertical
        && crop.millidegrees == 0
        && crop.left == 0
        && crop.top == 0
        && crop.width >= pixels.width()
        && crop.height >= pixels.height()
}

/// `crop`'s rectangle brought inside `pixels`, as `(left, top, width, height)`.
fn inside(pixels: &DynamicImage, crop: Crop) -> (u32, u32, u32, u32) {
    // There is no out-of-bounds refusal to add to three error types for a case no interface can produce: the canvas
    // a turn lands on is at most one pixel larger per axis than the space the widget measures in, so what this guards
    // is the process boundary and `Crop::scaled`'s rounding.
    //
    // The last pixel that exists on each axis, which is where an origin past the edge lands. A picture
    // with no pixels on an axis is not something a decoder produces; saturating rather than asserting it
    // keeps this total, which is the whole point of the clamp.
    let left = crop.left.min(pixels.width().saturating_sub(1));
    let top = crop.top.min(pixels.height().saturating_sub(1));

    (
        left,
        top,
        crop.width.min(pixels.width() - left).max(1),
        crop.height.min(pixels.height() - top).max(1),
    )
}

/// What identifies the pixels `crop` frames out of the picture identified by `source`.
///
/// Every field of the crop is folded in, so two framings of one photograph are two identities and the
/// same framing twice is one. See [`DISCRIMINATOR`].
fn identity_of(source: &str, crop: Crop) -> String {
    rust_sak::crypto::xxh3_string(&format!(
        "{source}|{DISCRIMINATOR}|{}|{}{}|{}|{}|{}|{}",
        crop.millidegrees, crop.flip_horizontal, crop.flip_vertical, crop.left, crop.top, crop.width, crop.height
    ))
}

/// The admitted file `identity` names, at `path`, decoded and framed as `crop` describes, off the runtime's own
/// threads.
///
/// # Errors
///
/// The loader's own, for a file that could no longer be read. A framing cannot fail.
///
/// # Panics
///
/// Where the framing itself panics.
pub(crate) async fn load_framed(
    opened: &Opened,
    identity: &str,
    path: PathBuf,
    crop: Option<Crop>,
) -> Result<Picture, opai::image::ImageIoError> {
    // **Here rather than beside either caller.** `crate::enhance::run` loads and frames its chain's input this way
    // and `crate::faces` a detection's on the same terms, so beside either they would be one sequence written twice.
    // Each caller still maps a failure into its own error, because each names what it was reading for.
    //
    // Through the decoded cache `opened` keeps, which the canvas's own rendition has usually filled already.
    let picture = opened.decoded().load(identity.to_string(), path).await?;

    let Some(crop) = crop else {
        return Ok(picture);
    };

    // `cropped` is blocking, and a framing on an inference path is the expensive one: a sixty-megapixel source is 240
    // million samples before the first model runs. So it is handed to a blocking thread here, exactly as
    // `opai::image::load` hands its decode to one, rather than occupying a runtime worker for the length of a warp.
    //
    // A join failure is a panic inside the framing and nothing else — the task is never cancelled, because nothing
    // drops this handle — so it is raised rather than turned into an error a window would have to draw a sentence
    // for. Inline, the same panic would have unwound the same way.
    Ok(crate::task::spawn_blocking(move || cropped(&picture, crop))
        .await
        .expect("framing a picture cannot fail, and nothing cancels its thread"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};

    use super::*;

    /// A framing of the whole of a 100x50 picture, unturned and unflipped, which every test below varies
    /// one field of.
    fn whole() -> Crop {
        Crop::new(0, 0, 100, 50, 0, false, false).expect("a rectangle with area is a legal crop")
    }

    #[test]
    fn a_rectangle_with_no_area_is_refused() {
        assert!(Crop::new(0, 0, 0, 50, 0, false, false).is_none(), "a zero-width rectangle frames nothing");
        assert!(Crop::new(0, 0, 100, 0, 0, false, false).is_none(), "a zero-height rectangle frames nothing");
        assert!(
            Crop::new(0, 0, 0, 0, 0, false, false).is_none(),
            "a rectangle with no area at all frames nothing"
        );

        // One pixel is the smallest thing that *is* a framing, and is not refused.
        assert!(Crop::new(0, 0, 1, 1, 0, false, false).is_some(), "a single pixel is a framing");
    }

    #[test]
    fn every_field_is_reported_back_as_given() {
        let crop = Crop::new(10, 20, 30, 40, -1_500, true, false).expect("a rectangle with area is a legal crop");

        assert_eq!(
            (crop.left, crop.top, crop.width(), crop.height()),
            (10, 20, 30, 40),
            "the rectangle came back as something other than what was asked for"
        );
        assert_eq!(crop.millidegrees, -1_500, "the turn came back as something other than what was asked for");
        assert!(crop.flip_horizontal, "the horizontal flip was dropped");
        assert!(!crop.flip_vertical, "a vertical flip was invented");
    }

    #[test]
    fn a_crop_is_a_key() {
        // The property both cache keys need, asserted by actually using one as a key rather than by
        // naming the traits: `Renditions` is a `HashMap` keyed on what was asked for, and the crop is
        // half of that.
        let mut framings: HashMap<Crop, &str> = HashMap::new();

        framings.insert(whole(), "the whole photograph");
        framings.insert(Crop::new(10, 0, 90, 50, 0, false, false).expect("legal"), "its right-hand side");

        assert_eq!(framings.get(&whole()), Some(&"the whole photograph"), "an equal crop did not find its entry");
        assert_eq!(framings.len(), 2, "two different framings collapsed into one entry");

        // One field apart is a different key, for each field in turn — which is what keeps a framing the
        // user changed from being served the one they had.
        for different in [
            Crop::new(1, 0, 100, 50, 0, false, false),
            Crop::new(0, 1, 100, 50, 0, false, false),
            Crop::new(0, 0, 99, 50, 0, false, false),
            Crop::new(0, 0, 100, 49, 0, false, false),
            Crop::new(0, 0, 100, 50, 1, false, false),
            Crop::new(0, 0, 100, 50, 0, true, false),
            Crop::new(0, 0, 100, 50, 0, false, true),
        ] {
            let different = different.expect("a rectangle with area is a legal crop");

            assert_ne!(different, whole(), "two framings that differ compared equal");
            assert!(!framings.contains_key(&different), "a framing found an entry belonging to another");
        }
    }

    #[test]
    fn the_turn_is_read_back_in_degrees() {
        let turned =
            |millidegrees| Crop::new(0, 0, 100, 50, millidegrees, false, false).expect("legal").rotation_degrees();

        assert!((turned(0) - 0.0).abs() < f64::EPSILON, "no turn read as one");
        assert!((turned(90_000) - 90.0).abs() < f64::EPSILON, "a quarter turn read as something else");
        // Negative, because the dialog turns both ways and a sign lost here is a photograph turned the
        // wrong way with nothing to say so.
        assert!((turned(-1_500) - -1.5).abs() < f64::EPSILON, "a counter-clockwise turn lost its sign");
    }

    #[test]
    fn scaling_moves_the_rectangle_and_nothing_else() {
        let crop = Crop::new(40, 20, 100, 50, -1_500, true, true).expect("legal");
        let half = crop.scaled(0.5);

        assert_eq!(
            (half.left, half.top, half.width(), half.height()),
            (20, 10, 50, 25),
            "the rectangle was not scaled into the reduced coordinate space"
        );

        // The reason the flips and the turn are left alone: neither means anything different at another
        // size, and scaling an angle would be turning the photograph by a fraction of what was asked.
        assert_eq!(half.millidegrees, crop.millidegrees, "scaling changed the turn");
        assert_eq!(half.flip_horizontal, crop.flip_horizontal, "scaling changed the horizontal flip");
        assert_eq!(half.flip_vertical, crop.flip_vertical, "scaling changed the vertical flip");

        // A factor of one is the unbounded request's path, and it has to be exactly the crop it was given
        // — that is what makes an unbounded rendition's dimensions the rectangle's own.
        assert_eq!(crop.scaled(1.0), crop, "scaling by one changed the framing");
    }

    #[test]
    fn a_rectangle_scaled_to_nothing_keeps_a_pixel_on_each_edge() {
        // A 4x2 rectangle in a miniature of a 60-megapixel photograph reduces to well under a pixel, and
        // the type still cannot describe nothing.
        let vanishing = Crop::new(1_000, 500, 4, 2, 0, false, false).expect("legal").scaled(0.001);

        assert_eq!((vanishing.width(), vanishing.height()), (1, 1), "a framing was scaled out of existence");
        assert_eq!((vanishing.left, vanishing.top), (1, 1), "the origin was not scaled with the rectangle");
    }
    /// A picture whose every pixel is distinguishable from every other, so a flip, a turn or a cut that
    /// went the wrong way shows up as a different picture rather than as the same one.
    fn picture_of(width: u32, height: u32) -> Picture {
        let pixels = RgbImage::from_fn(width, height, |x, y| Rgb([x as u8, y as u8, 7]));

        Picture::new("/pictures/holiday.jpg", DynamicImage::ImageRgb8(pixels), "5ec0ffee5ec0ffee")
    }

    #[test]
    fn the_order_is_flip_flip_turn_cut() {
        let source = picture_of(4, 2);

        // Composed here in the order this module documents, out of the same four primitives, so what is
        // pinned is the order rather than the operations.
        let expected = rust_sak::image::rotate(&source.pixels().fliph().flipv(), 90.0).crop_imm(0, 1, 2, 3);

        let framed = cropped(&source, Crop::new(0, 1, 2, 3, 90_000, true, true).expect("legal"));

        assert_eq!(framed.pixels(), &expected, "the framing was not flip, flip, turn, cut");
        assert_eq!(framed.dimensions(), (2, 3), "the cut did not answer the rectangle's own dimensions");
    }

    #[test]
    fn cutting_before_flipping_is_the_same_pixels() {
        // The unturned path cuts the mirrored rectangle out of the source and flips only the cut. This is the
        // composition it replaced — flip the whole photograph, then cut — for every combination of flips, over a
        // picture and rectangles that are asymmetric on both axes so a mirror taken the wrong way cannot agree by
        // accident. The last rectangle runs past both edges, so the clamp is covered too.
        let source = picture_of(37, 23);

        for (flip_horizontal, flip_vertical) in [(false, false), (true, false), (false, true), (true, true)] {
            for (left, top, width, height) in [(3, 5, 11, 7), (0, 0, 36, 22), (30, 1, 20, 40)] {
                let crop = Crop::new(left, top, width, height, 0, flip_horizontal, flip_vertical).expect("legal");

                let mut flipped = source.pixels().clone();
                if flip_horizontal {
                    flipped = flipped.fliph();
                }
                if flip_vertical {
                    flipped = flipped.flipv();
                }
                let (cut_left, cut_top, cut_width, cut_height) = inside(&flipped, crop);
                let expected = flipped.crop_imm(cut_left, cut_top, cut_width, cut_height);

                assert_eq!(
                    frame(source.pixels(), crop).as_ref(),
                    &expected,
                    "flips {flip_horizontal}/{flip_vertical} over {left},{top} {width}x{height} framed other pixels"
                );
            }
        }
    }

    #[test]
    fn turning_happens_after_the_flips_and_not_before() {
        let source = picture_of(4, 2);

        // One flip and a quarter turn, which is the case that distinguishes the two orders: both flips
        // together are a half turn, and a half turn commutes with every other turn.
        let flipped_first = rust_sak::image::rotate(&source.pixels().fliph(), 90.0);
        let turned_first = rust_sak::image::rotate(source.pixels(), 90.0).fliph();

        assert_ne!(flipped_first, turned_first, "the fixture cannot tell the two orders apart");

        let framed = cropped(&source, Crop::new(0, 0, 2, 4, 90_000, true, false).expect("legal"));

        assert_eq!(framed.pixels(), &flipped_first, "the photograph was turned before it was flipped");
    }

    #[test]
    fn a_positive_angle_turns_clockwise() {
        let source = picture_of(2, 2);
        let corner = source.pixels().get_pixel(0, 1);

        // A quarter turn clockwise brings the bottom-left corner to the top left. A quarter turn is exact
        // — no interpolation — so the pixel that lands there is the one that was there, not a blend.
        let framed = cropped(&source, Crop::new(0, 0, 2, 2, 90_000, false, false).expect("legal"));

        assert_eq!(
            framed.pixels().get_pixel(0, 0),
            corner,
            "a positive angle turned the photograph counter-clockwise"
        );
    }

    #[test]
    fn a_rectangle_running_past_the_edge_frames_what_exists() {
        let source = picture_of(100, 50);

        // Fifty wide and fifty tall asked for, ten by ten actually there.
        let framed = cropped(&source, Crop::new(90, 40, 50, 50, 0, false, false).expect("legal"));

        assert_eq!(framed.dimensions(), (10, 10), "a rectangle past the edge was not brought inside the picture");
    }

    #[test]
    fn a_rectangle_wholly_outside_frames_the_nearest_pixel_that_exists() {
        let source = picture_of(100, 50);
        let last = source.pixels().get_pixel(99, 49);

        let framed = cropped(&source, Crop::new(500, 500, 10, 10, 0, false, false).expect("legal"));

        // One pixel on each edge, which is the floor, and the pixel it answers is the one nearest what was
        // asked for.
        assert_eq!(framed.dimensions(), (1, 1), "a rectangle wholly outside the picture did not answer one pixel");
        assert_eq!(framed.pixels().get_pixel(0, 0), last, "the framing answered a pixel other than the nearest");
    }

    #[test]
    fn a_sixteen_bit_photograph_stays_sixteen_bit() {
        let deep = Picture::new(
            "/pictures/sunset.dng",
            DynamicImage::ImageRgb16(ImageBuffer::from_fn(8, 4, |x, y| Rgb([x as u16 * 8_000, y as u16 * 8_000, 7]))),
            "cafebabecafebabe",
        );

        let cut = cropped(&deep, Crop::new(1, 1, 4, 2, 0, false, false).expect("legal"));
        assert_eq!(cut.pixels().color(), image::ColorType::Rgb16, "a developed RAW was flattened by being cut");

        // A turn promotes to the alpha-bearing sibling, because it exposes corners no source pixel covers
        // — and the sibling of sixteen-bit RGB is sixteen-bit RGBA, not eight-bit anything.
        let turned = cropped(&deep, Crop::new(0, 0, 8, 4, 30_000, false, false).expect("legal"));
        assert_eq!(
            turned.pixels().color(),
            image::ColorType::Rgba16,
            "a developed RAW was flattened by being turned"
        );
    }

    #[test]
    fn a_corner_a_turn_exposes_is_transparent() {
        let source = picture_of(100, 50);

        // The whole of the turned canvas, which is what a deliberately over-dragged rectangle reaches: the
        // rectangle is clamped to the canvas the turn landed on, corners included.
        let framed = cropped(&source, Crop::new(0, 0, 10_000, 10_000, 45_000, false, false).expect("legal"));

        assert!(framed.pixels().color().has_alpha(), "a turned photograph came back with no alpha channel");
        assert_eq!(framed.pixels().get_pixel(0, 0)[3], 0, "a corner no source pixel covers was given a colour");
    }

    #[test]
    fn a_framing_does_not_carry_its_sources_identity() {
        let source = picture_of(100, 50);
        let framed = cropped(&source, Crop::new(10, 10, 20, 20, 0, false, false).expect("legal"));

        // What makes a framed picture a distinct input to the run cache `opai::process` keys on. Were it
        // to keep the source's identity, the chain's cached result for the whole photograph would be
        // served for the framing.
        assert_ne!(framed.identity(), source.identity(), "a framing was named after the photograph it came from");
        assert_eq!(framed.path(), source.path(), "the framing forgot where its pixels came from");
    }

    #[test]
    fn two_framings_of_one_photograph_are_two_identities() {
        let source = picture_of(100, 50);

        let left = cropped(&source, Crop::new(0, 0, 50, 50, 0, false, false).expect("legal"));
        let right = cropped(&source, Crop::new(50, 0, 50, 50, 0, false, false).expect("legal"));

        assert_ne!(left.identity(), right.identity(), "two framings of one photograph answered one identity");

        // Every field of the crop is folded in, so a change to any one of them is a different input.
        for other in [
            Crop::new(0, 0, 50, 50, 1, false, false),
            Crop::new(0, 0, 50, 50, 0, true, false),
            Crop::new(0, 0, 50, 50, 0, false, true),
            Crop::new(0, 1, 50, 50, 0, false, false),
            Crop::new(0, 0, 50, 49, 0, false, false),
        ] {
            let other = cropped(&source, other.expect("legal"));

            assert_ne!(other.identity(), left.identity(), "two framings that differ answered one identity");
        }
    }

    #[test]
    fn the_same_framing_twice_is_one_identity() {
        let source = picture_of(100, 50);
        let crop = Crop::new(10, 10, 20, 20, -1_500, true, false).expect("legal");

        // Stable rather than merely unique: a re-run of one chain over one framing has to find what the
        // first run cached, which it can only do if the input is named the same way both times.
        assert_eq!(
            cropped(&source, crop).identity(),
            cropped(&source, crop).identity(),
            "one framing of one photograph answered two identities"
        );
    }

    #[test]
    fn a_framing_that_changes_nothing_answers_the_source_untouched() {
        let source = picture_of(100, 50);

        for whole in [
            // The rectangle the dialog reports for a photograph nothing has been done to.
            Crop::new(0, 0, 100, 50, 0, false, false),
            // And one larger than the picture, which the cut would clamp back to exactly the same pixels.
            Crop::new(0, 0, 200, 100, 0, false, false),
        ] {
            let framed = cropped(&source, whole.expect("legal"));

            assert_eq!(framed.identity(), source.identity(), "a framing that changed nothing minted a new identity");
            assert!(
                Arc::ptr_eq(&framed.shared_pixels(), &source.shared_pixels()),
                "a framing that changed nothing copied the photograph"
            );
        }
    }
}
