//! The pixel primitives every model is built from, and the loop that runs a fixed-size tile over an image of any
//! size.
//!
//! Every tiled model Open Photo AI ships takes one shape — `1x3x256x256` `f32`, planar, channel-major, RGB,
//! normalised — every whole-image model takes a fixed square of its own, and no photograph has either. What crosses
//! that gap in both directions lives here: where the tiles are, how a region of pixels becomes the numbers a model
//! reads, how the numbers it returns become pixels again and at what depth, and how overlapping results are combined
//! into one picture with no visible seam.
//!
//! That is the tile grid ([`grid`]), the padding rule ([`pad`]), the tensor conversions ([`tensor`]), both blending
//! strategies — write-over for a deterministic model ([`blend`]), average-every-contribution for a stochastic one
//! ([`canvas`]) — the fixed-canvas plan and presentation a whole-image run reaches its graph through ([`present`]), the
//! per-run control blend that moves a photograph towards a finished result ([`mix`]), the align-centres bilinear read
//! of a graph's square plane back onto the photograph ([`bilinear`]), the row-banded split of a full-resolution CPU
//! pass across cores ([`rows`]), and the tiling driver [`run_tiled`].
//!
//! **This crate knows nothing of `opai`**: no model, session, execution provider or orchestration. It depends on
//! `image` and `thiserror` alone, so that independence is a compile error to break rather than a convention to keep.
//! Its items are `pub` because that is how `opai` reaches them; it is not published.

// A crate rather than a module of `opai` for exactly that reason. Everything here is a leaf — it imports only `image`,
// `std` and its own siblings — while the pipeline contract models implement names sessions, providers and artifacts,
// so the contract stays in `opai` and only the leaves cross the boundary.
//
// Shared by all eight model families rather than written inside whichever lands first: written inside one pipeline it
// would be rewritten inside the next, and the two would disagree.
//
// The tiling loop needs no runtime, GPU, model file, filesystem or network: the per-tile step is a parameter of
// `run_tiled`, so the whole loop — the partitioning, the padding, the progress accounting, the blending and the
// refusals — is exercised on every CI platform against a tile function that is arithmetic.
//
// What is deliberately not here:
//
// - Resampling a tiled run. The driver never resizes: an upscale that overshoots — a 1.5x request served by a 2x
//   pass — is corrected by the pipeline, and the diffusion variant's fit-to-alignment resizes are its own. The driver
//   only ever crops. The one resample here is `present`'s, which fits a photograph onto a whole-image graph's square.
// - The diffusion upscaler's own driver. It needs this same partitioning with a completely different per-tile
//   pipeline — three sessions rather than one, `[-1, 1]` rather than `[0, 1]`, no output scale at all, and an
//   alignment to a multiple of 16 rather than to the tile shape — which is why `TileGrid`, `pad` and the ramp are
//   usable without `run_tiled`. The driver is one model's, so it lives with that model in `opai`.
// - Logging. The reference logs per-pass timings around its driver; each fact is carried as data here instead —
//   progress through the callback, failure through `TilingError`.

pub mod bilinear;
pub mod blend;
pub mod canvas;
pub mod grid;
pub mod mix;
pub mod pad;
pub mod present;
pub mod rows;
pub mod tensor;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use image::{DynamicImage, ImageBuffer, Rgb};
use thiserror::Error;

use blend::blend_tile;
use grid::TileGrid;
use tensor::{Channel, Normalisation, Sampler, chw_to_image, image_to_chw};

/// The tile shape a run partitions an image into, and how far each tile is asked to overlap the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileGeometry {
    /// The side of the square shape the model accepts.
    pub size: u32,
    /// How far each tile is asked to overlap its predecessor. What a tile *actually* overlaps is the layout's to
    /// report — see [`grid::Layout::overlaps_x`].
    pub overlap: u32,
}

impl TileGeometry {
    // A named default rather than a constant inside the driver, because a model that needs its own drives `TileGrid`
    // directly and should not have to restate this one to say it is not using it. A `const` as well as the `Default`
    // below, so that a model's row — a `static` — can point at it rather than respell the two numbers.
    /// The geometry every fixed-shape model this project ships was tuned against.
    pub const DEFAULT: Self = Self { size: 256, overlap: 16 };
}

impl Default for TileGeometry {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// A named type rather than the closure spelt out at each of the places it appears — and the out-parameter is
// deliberate: a `Vec` returned per tile is thousands of identically-shaped allocations, all immediately garbage.
/// The per-tile step a run is given: the tile already converted and padded, and the buffer the model's output is to
/// be written into.
pub type RunTile<'a, E> = dyn FnMut(&[f32], &mut [f32]) -> Result<(), E> + 'a;

/// How many bits per channel a result carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelDepth {
    /// Eight bits per channel.
    Eight,
    /// Sixteen bits per channel.
    Sixteen,
}

impl ChannelDepth {
    /// How this depth is spelled inside a composed identity, which the image cache is keyed on.
    pub fn tag(self) -> &'static str {
        // Written out rather than reached for through the derived `Debug`: a derived formatting is a debugging aid
        // nobody owes stability to, and a rename of a variant would silently re-key every entry on disk.
        match self {
            Self::Eight => "8",
            Self::Sixteen => "16",
        }
    }
}

/// A tiled run that did not produce a picture.
///
/// Generic in the per-tile function's own error so that nothing is boxed and a caller's failure — a session that
/// could not run — arrives intact rather than as a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TilingError<E> {
    // A refusal rather than an empty success. A caller handed an untouched buffer with no error has no way to tell it
    // from a finished enhancement, and would show it to a user as one.
    /// The image has no area, so there is no partitioning of it and nothing to run.
    #[error("cannot tile a {width}x{height} image")]
    Untileable {
        /// The width that was asked for.
        width: u32,
        /// The height that was asked for.
        height: u32,
    },

    /// The caller asked the run to stop, and it stopped between tiles.
    ///
    /// **Deliberately not the core library's `task::Cancelled`**, which means the async runtime shut down before
    /// blocking work could run. One is a user closing a dialog and the other is a process on its way down; they are
    /// reported to a user differently, and a caller that conflated them would tell someone their enhancement failed
    /// when they had just cancelled it themselves.
    #[error("the run was cancelled before it finished")]
    Cancelled,

    /// The per-tile function failed, and the whole run fails with it.
    #[error("the model failed on tile {index}")]
    Tile {
        // Carried because a failure on the first tile of an image and a failure on its four hundredth are different
        // reports: one is a model that cannot run at all, the other is something about that region.
        /// Which tile it was, counting from zero in the row-major order [`grid::Layout::tiles`] produces.
        index: usize,
        /// What the per-tile function reported.
        #[source]
        source: E,
    },
}

/// Runs `run_tile` over `source` one fixed-size tile at a time and stitches the results into one picture.
///
/// `scale` is the model's output scale — 1 for a model that returns an image the size of its input, N for an NxN
/// upscale — so the result is the source's dimensions multiplied by it.
///
/// The **caller** emits the initial `progress(0.0)`: a multi-pass upscale calls this once per pass and must not
/// reset to zero at the start of each.
///
/// # Errors
///
/// Returns [`TilingError::Untileable`] for an image with no area, before anything is allocated;
/// [`TilingError::Cancelled`] when `cancelled` answers true before a tile; and [`TilingError::Tile`] naming the tile
/// when `run_tile` fails. A failed or cancelled run returns **no image at all**: the half-written buffer is not a
/// partial result.
#[expect(
    clippy::too_many_arguments,
    reason = "design D2's parameter list; grouping it would only move the same fields into a struct each call site               builds inline"
)]
pub fn run_tiled<E>(
    source: &DynamicImage,
    geometry: TileGeometry,
    scale: u32,
    depth: ChannelDepth,
    norm: Normalisation,
    progress: Option<&dyn Fn(f64)>,
    cancelled: &dyn Fn() -> bool,
    run_tile: &mut RunTile<'_, E>,
) -> Result<DynamicImage, TilingError<E>> {
    match depth {
        ChannelDepth::Eight => {
            tiled::<u8, E>(source, geometry, scale, norm, progress, cancelled, run_tile).map(u8::into_dynamic)
        }
        ChannelDepth::Sixteen => {
            tiled::<u16, E>(source, geometry, scale, norm, progress, cancelled, run_tile).map(u16::into_dynamic)
        }
    }
}

/// [`run_tiled`]'s body, once, at whichever channel the caller asked for.
fn tiled<T: Channel, E>(
    source: &DynamicImage,
    geometry: TileGeometry,
    scale: u32,
    norm: Normalisation,
    progress: Option<&dyn Fn(f64)>,
    cancelled: &dyn Fn() -> bool,
    run_tile: &mut RunTile<'_, E>,
) -> Result<ImageBuffer<Rgb<T>, Vec<T>>, TilingError<E>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    let (width, height) = (source.width(), source.height());

    // Before any buffer is allocated, and before the division that computes a tile's share of the progress.
    if width == 0 || height == 0 {
        return Err(TilingError::Untileable { width, height });
    }

    // A model returns at least the pixels it was given, so a scale of zero has no meaning; it would make the result
    // empty, which is the one outcome this function exists to refuse.
    let scale = scale.max(1);

    // A geometry that cannot partition anything — a zero tile, or an overlap that leaves no forward progress — is a
    // mistake in a model's profile rather than anything a user can cause, so it is caught in the tests that run
    // against every profile. In a release build it is normalised instead of aborting an export: the alternative is a
    // grid whose single tile is larger than the shape the model accepts, which quietly leaves most of the image
    // unwritten.
    debug_assert!(
        geometry.size > 0 && geometry.overlap < geometry.size,
        "a {}/{} geometry partitions nothing",
        geometry.size,
        geometry.overlap
    );

    let shape = geometry.size.max(1);
    let overlap = geometry.overlap.min(shape - 1);

    let layout = TileGrid { size: shape, overlap, width, height }.layout();
    let tiles = layout.tiles();

    if tiles.is_empty() {
        return Err(TilingError::Untileable { width, height });
    }

    let columns = layout.columns();
    // Per axis, because how far a tile overlaps its neighbour depends on where the grid put it, not on what the
    // geometry asked for. Ramping over the configured overlap instead leaves a hard edge down the last column of the
    // picture and along its last row, which is a bug the reference ships. See `Layout::overlaps_x`.
    let (overlaps_x, overlaps_y) = (layout.overlaps_x(), layout.overlaps_y());

    // The scratch, allocated once for the whole run. Every tile is converted at the one shape the model accepts —
    // the grid moves a tile that would overhang back rather than shrinking it, and the single tile of an image
    // smaller than one is mirrored out to the shape — which is what makes one of each enough. At 256/16 a
    // 24-megapixel photograph is over four hundred tiles, each of whose output buffer is 12 MB at a 4x pass;
    // allocating per tile is gigabytes of identically-shaped garbage.
    let out_shape = shape * scale;

    let sampler = Sampler::new(source);
    let mut input = vec![0.0_f32; 3 * (shape as usize) * (shape as usize)];
    let mut output = vec![0.0_f32; 3 * (out_shape as usize) * (out_shape as usize)];
    let mut decoded = ImageBuffer::<Rgb<T>, Vec<T>>::new(out_shape, out_shape);
    let mut result = ImageBuffer::<Rgb<T>, Vec<T>>::new(width * scale, height * scale);

    let step = 1.0 / tiles.len() as f64;
    let mut reported = 0.0;

    for (index, tile) in tiles.iter().enumerate() {
        if cancelled() {
            return Err(TilingError::Cancelled);
        }

        // The scratch is allocated at exactly this shape above, so the length can never disagree.
        image_to_chw(&mut input, &sampler, *tile, shape, norm).expect("the scratch is allocated at the tile shape");

        run_tile(&input, &mut output).map_err(|source| TilingError::Tile { index, source })?;

        chw_to_image(&mut decoded, &output, norm).expect("the scratch is allocated at the tile shape");

        // The padding is cropped back off here, at the model's scale: `result` is the source's own dimensions
        // multiplied by the scale, so a decoded tile that runs past them is written only as far as they go.
        blend_tile(
            &mut result,
            &decoded,
            tile.x * scale,
            tile.y * scale,
            overlaps_x[index % columns] * scale,
            overlaps_y[index / columns] * scale,
        );

        if let Some(report) = progress {
            reported += step;

            // The accumulated sum of four hundred reciprocals does not land on 1, and a progress bar that stops at
            // 99.98% is a bug report. The last tile reports exactly 1 rather than whatever the sum reached.
            report(if index + 1 == tiles.len() { 1.0 } else { reported.min(1.0) });
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use image::GenericImageView as _;

    use super::*;
    use crate::test_support::{self, gradient};

    /// A per-tile function that copies its input straight through — the model that changes nothing, against which a
    /// result that differs from its source is the driver's own doing.
    fn identity(input: &[f32], output: &mut [f32]) -> Result<(), Infallible> {
        output.copy_from_slice(input);
        Ok(())
    }

    /// The shared block-enlarging model as a per-tile function. The factor is the one the scratch pair implies, so
    /// `scale` is here only to keep the call sites reading as the run they describe.
    fn nearest_neighbour(_scale: u32) -> impl FnMut(&[f32], &mut [f32]) -> Result<(), Infallible> {
        |input: &[f32], output: &mut [f32]| {
            test_support::nearest_neighbour(input, output);
            Ok(())
        }
    }

    /// Runs a source through the driver with nothing watching it, at the default geometry.
    fn run(
        source: &DynamicImage,
        scale: u32,
        depth: ChannelDepth,
        run_tile: &mut RunTile<'_, Infallible>,
    ) -> Result<DynamicImage, TilingError<Infallible>> {
        run_tiled(source, TileGeometry::default(), scale, depth, Normalisation::Unit, None, &|| false, run_tile)
    }

    #[test]
    fn a_multi_tile_image_comes_back_unchanged_through_a_model_that_changes_nothing() {
        // 600x400 at 256/16 is three columns by two rows, including the moved final column and row whose overlap is
        // 152 and 112 rather than 16 — so this is also the check that a wide seam reproduces the source exactly
        // rather than merely smoothly.
        let source = gradient(600, 400);
        let result = run(&source, 1, ChannelDepth::Eight, &mut identity).unwrap();

        assert_eq!(result.dimensions(), (600, 400));

        let result = result.to_rgb8();
        let expected = source.to_rgb8();

        for y in 0..400 {
            for x in 0..600 {
                assert_eq!(result.get_pixel(x, y), expected.get_pixel(x, y), "the result differs at ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_model_that_enlarges_produces_a_result_at_its_own_scale() {
        let source = gradient(300, 140);
        let result = run(&source, 4, ChannelDepth::Eight, &mut nearest_neighbour(4)).unwrap();

        assert_eq!(result.dimensions(), (1200, 560));

        // Each source pixel became a 4x4 block, so the corners of a block are the pixel it came from.
        let result = result.to_rgb8();
        let source = source.to_rgb8();

        for (x, y) in [(0, 0), (37, 61), (299, 139)] {
            for (dx, dy) in [(0, 0), (3, 3)] {
                assert_eq!(
                    result.get_pixel(x * 4 + dx, y * 4 + dy),
                    source.get_pixel(x, y),
                    "the block for ({x}, {y}) is not the pixel it came from"
                );
            }
        }
    }

    #[test]
    fn an_image_smaller_than_one_tile_loses_its_padding_from_the_result() {
        // One tile, mirrored out to 256 square before the model sees it. What comes back is the image, not the tile.
        let source = gradient(40, 17);

        let result = run(&source, 1, ChannelDepth::Eight, &mut identity).unwrap();
        assert_eq!(result.dimensions(), (40, 17));
        assert_eq!(result.to_rgb8().as_raw(), source.to_rgb8().as_raw(), "the padding reached the result");

        // And at a scale, where the crop has to happen at the model's own scale rather than the source's.
        let enlarged = run(&source, 4, ChannelDepth::Eight, &mut nearest_neighbour(4)).unwrap();
        assert_eq!(enlarged.dimensions(), (160, 68));
    }

    #[test]
    fn the_result_carries_the_depth_that_was_asked_for() {
        let source = gradient(300, 300);

        let eight = run(&source, 1, ChannelDepth::Eight, &mut identity).unwrap();
        let sixteen = run(&source, 1, ChannelDepth::Sixteen, &mut identity).unwrap();

        assert!(matches!(eight, DynamicImage::ImageRgb8(_)));
        assert!(matches!(sixteen, DynamicImage::ImageRgb16(_)));
        assert_eq!(sixteen.dimensions(), (300, 300));

        // The same picture at both depths: an 8-bit source widens exactly, so the 16-bit result is the 8-bit one
        // scaled by 257 rather than anything new.
        let (eight, sixteen) = (eight.to_rgb8(), sixteen.to_rgb16());
        for (a, b) in eight.pixels().zip(sixteen.pixels()) {
            for channel in 0..3 {
                assert_eq!(u16::from(a.0[channel]) * 257, b.0[channel]);
            }
        }
    }

    #[test]
    fn the_scratch_is_allocated_once_however_many_tiles_run() {
        // 2000x2000 at 256/16 is 81 tiles. If either buffer were allocated per tile the addresses it is handed would
        // change between them; one allocation per run means every tile sees the same two.
        let source = gradient(2000, 2000);

        let mut buffers = Vec::new();
        let mut tiles = 0;

        run(&source, 1, ChannelDepth::Eight, &mut |input: &[f32], output: &mut [f32]| {
            buffers.push((input.as_ptr() as usize, output.as_ptr() as usize, input.len(), output.len()));
            tiles += 1;
            output.copy_from_slice(input);
            Ok(())
        })
        .unwrap();

        assert_eq!(tiles, 81, "the grid over a 2000x2000 image is not the 81 tiles this asserts against");

        let first = buffers[0];
        assert!(buffers.iter().all(|seen| *seen == first), "the scratch was reallocated between tiles");
        assert_eq!(first.2, 3 * 256 * 256, "the input scratch is not one tile's worth of floats");
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
    #[error("the session refused the tile")]
    struct SessionFailed;

    #[test]
    fn an_image_with_no_area_is_refused_before_anything_is_allocated() {
        // All three zero-area shapes. Each would otherwise partition into nothing, run the loop zero times, and hand
        // back a freshly allocated buffer that a caller cannot tell from a finished enhancement.
        for (width, height) in [(0, 100), (100, 0), (0, 0)] {
            let source = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let mut ran = false;
            let error = run(&source, 1, ChannelDepth::Eight, &mut |input: &[f32], output: &mut [f32]| {
                ran = true;
                output.copy_from_slice(input);
                Ok(())
            })
            .unwrap_err();

            assert_eq!(error, TilingError::Untileable { width, height });
            assert!(!ran, "a {width}x{height} image reached the model");
        }
    }

    #[test]
    fn progress_never_decreases_and_ends_at_exactly_one() {
        // 5120x5120 at 256/16 is 22 columns by 22 rows — 484 tiles, whose reciprocals summed do not land on 1.
        let source = gradient(5120, 5120);

        let reported = std::cell::RefCell::new(Vec::new());
        let progress = |value: f64| reported.borrow_mut().push(value);

        run_tiled(
            &source,
            TileGeometry::default(),
            1,
            ChannelDepth::Eight,
            Normalisation::Unit,
            Some(&progress),
            &|| false,
            &mut identity,
        )
        .unwrap();

        let reported = reported.into_inner();

        assert_eq!(reported.len(), 484, "one report per tile is not what was emitted");
        assert!(reported[0] > 0.0, "the initial zero is the caller's to emit, not the driver's");
        assert!((reported[0] - 1.0 / 484.0).abs() < 1e-12, "the first tile did not advance by one tile's share");

        for pair in reported.windows(2) {
            assert!(pair[1] >= pair[0], "progress went backwards: {pair:?}");
        }

        assert_eq!(*reported.last().unwrap(), 1.0, "the final report was not exactly 1");
    }

    #[test]
    fn a_run_nobody_is_watching_produces_exactly_what_a_watched_one_does() {
        // The progress parameter is an `Option`, so a caller that does not ask for progress is charged no callback
        // and not even the addition behind it — which is structural rather than observable from out here. What a
        // test can hold is the other half of that promise: reporting is the *only* difference between the two runs,
        // so nothing about the picture depends on whether anyone was watching.
        let source = gradient(600, 400);

        let reported = std::cell::RefCell::new(Vec::new());
        let progress = |value: f64| reported.borrow_mut().push(value);

        let watched = run_tiled(
            &source,
            TileGeometry::default(),
            1,
            ChannelDepth::Eight,
            Normalisation::Unit,
            Some(&progress),
            &|| false,
            &mut identity,
        )
        .unwrap();

        let unwatched = run(&source, 1, ChannelDepth::Eight, &mut identity).unwrap();

        assert_eq!(
            watched.to_rgb8().as_raw(),
            unwatched.to_rgb8().as_raw(),
            "the picture came out differently depending on who was watching it"
        );

        // 600x400 at 256/16 is three columns by two rows, and the watched run pays one report for each of them.
        assert_eq!(reported.into_inner().len(), 6, "the watched run is not one report per tile");
    }

    #[test]
    fn a_cancelled_run_stops_before_the_next_tile_and_returns_no_picture() {
        let source = gradient(600, 400);

        let ran = std::cell::Cell::new(0);
        let stop_after_two = || ran.get() >= 2;

        let result = run_tiled(
            &source,
            TileGeometry::default(),
            1,
            ChannelDepth::Eight,
            Normalisation::Unit,
            None,
            &stop_after_two,
            &mut |input: &[f32], output: &mut [f32]| {
                ran.set(ran.get() + 1);
                output.copy_from_slice(input);
                Ok::<(), Infallible>(())
            },
        );

        assert_eq!(result.unwrap_err(), TilingError::Cancelled, "a cancelled run did not report being cancelled");
        assert_eq!(ran.get(), 2, "the cancellation was not consulted before the next tile");
    }

    #[test]
    fn a_tile_the_model_fails_on_fails_the_whole_run_and_names_it() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
        #[error("the graph rejected the tile")]
        struct GraphFailed;

        let source = gradient(600, 400);

        let mut ran = 0;
        let result = run_tiled(
            &source,
            TileGeometry::default(),
            1,
            ChannelDepth::Eight,
            Normalisation::Unit,
            None,
            &|| false,
            &mut |input: &[f32], output: &mut [f32]| {
                ran += 1;
                if ran == 4 {
                    return Err(GraphFailed);
                }
                output.copy_from_slice(input);
                Ok(())
            },
        );

        // No picture at all: the tiles that had already succeeded are not a partial result, because everything past
        // them is whatever the buffer was allocated as.
        assert_eq!(result.unwrap_err(), TilingError::Tile { index: 3, source: GraphFailed });
        assert_eq!(ran, 4, "the run continued past the tile that failed");
    }

    #[test]
    fn a_tile_failure_reports_the_tile_it_happened_at_and_carries_the_error_unboxed() {
        let error = TilingError::Tile { index: 399, source: SessionFailed };

        assert_eq!(error.to_string(), "the model failed on tile 399");

        // Unboxed: the caller's own error type comes back out, not a `Box<dyn Error>` it has to downcast.
        let TilingError::Tile { index, source } = error else { panic!("the variant changed") };
        assert_eq!(index, 399);
        assert_eq!(source, SessionFailed);

        // And it is reachable as the source of the failure, so a report that walks the chain reaches the session.
        assert_eq!(std::error::Error::source(&error).unwrap().to_string(), "the session refused the tile");
    }

    #[test]
    fn an_image_with_no_area_names_the_shape_that_was_refused() {
        let error = TilingError::<Infallible>::Untileable { width: 0, height: 512 };
        assert_eq!(error.to_string(), "cannot tile a 0x512 image");
    }
}
