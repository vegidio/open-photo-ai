//! The diffusion upscaler: resample to the target size, restore detail at that size, and put the colour back.
//!
//! Osaka is not a scaling model. The network is resolution-preserving throughout — the VAE compresses 8x and expands
//! 8x and the transformer between them does not change the token grid — so the image is resampled to its target size
//! *first* and detail is restored at that size. That is not a fallback for a model that cannot enlarge; it is how
//! SeedVR2 is meant to be driven, and the reference drives it the same way for the same reason.
//!
//! The pipeline, in order:
//!
//! 1. **Resample** to the requested scale with `Lanczos3` — the same filter the convolutional path corrects an
//!    overshoot with. At a scale of 1 this leaves the image alone, and the run is still a restoration.
//! 2. **Extend** to a multiple of 16 and to at least one region, by mirroring the image's own edge pixels. Nothing is
//!    materialised: the extension is a coordinate map, [`pad::source_offset`](imaging::pad::source_offset).
//! 3. **Restore** region by region, each exactly 960x960 through the three graphs — see [`region`].
//! 4. **Combine** every region into a weighted float accumulator, dividing once at the end — see [`canvas`](imaging::canvas).
//! 5. **Correct** the colour once, on the assembled image, against the resampled image the model was conditioned on —
//!    see [`colorfix`].
//! 6. **Reduce** to the caller's channel depth, once, and crop the extension off.
//!
//! # This directory is the whole of Osaka
//!
//! The row [`OSAKA`] and the two session constants behind it, the profile measured for it and the prose recording how
//! ([`profile`]), the precisions it is published in ([`precision`]), the vocabulary its three-graph contract needs
//! ([`graph`]), this driver, the helpers under it, and the tests for all of it. Deleting the model is deleting this
//! directory plus the arms in [`UpscaleVariant`](super::UpscaleVariant) that name it. Nothing else here belongs to
//! any other model, and nothing here reaches into one — see [`crate::models`] for the rule, and [`imaging`] and
//! [`crate::pipeline`] for what is shared instead.
//!
//! # Why this is a driver of its own rather than [`run_tiled`](imaging::run_tiled)
//!
//! Four of that driver's assumptions are hardwired and all four are wrong here. It drives a **single** session, where
//! a region is three; it feeds `[1,3,H,W]` in `[0,1]`, where this needs `[-1,1]` and two of its three graphs are
//! neither three-channel nor at the image's resolution; it takes an integer output scale, where this one preserves
//! resolution; and it pads to the tile size, where this must align to a multiple of 16 for the 8x VAE and the 2x
//! patchify on top of it. Adding four conditionals to the driver every other model takes, to serve one model, would
//! put this contract's edge cases in front of every convolutional run.
//!
//! What can be shared **is** shared, and it was factored out for this caller before this caller existed: the
//! partitioning ([`grid`](imaging::grid)), the reflection rule ([`pad`](imaging::pad)) and the ramp curve
//! ([`ramp_weight`](imaging::blend::ramp_weight)). What is left duplicated is a loop over tiles, which is the shape of
//! the thing rather than its substance.
//!
//! # There are two ways of combining tiles in this crate, and this is the second
//!
//! [`blend_tile`](imaging::blend::blend_tile) writes each tile over what is already in the picture, at the caller's
//! channel depth. That is right for a convolutional model, where two tiles covering one pixel very nearly agree — but
//! it can average two tiles and no more, so where four meet the result depends on the order they were written.
//!
//! [`canvas`](imaging::canvas) weights every contribution into a planar float sum and divides once. That is what a model drawing its
//! own noise per region needs, because its regions genuinely disagree about the pixels they share and that corner is
//! exactly where a seam shows. **Which one applies is decided by whether the per-tile output is stochastic**, not by
//! which pipeline is being written. Both feather with the same [`ramp_weight`](imaging::blend::ramp_weight) over the
//! width the two tiles actually share, so a tuning change cannot be applied to only one of them.
//!
//! # One deliberate divergence from the reference
//!
//! **Regions are feathered over the width they actually share, not over the configured 128.** The reference ramps
//! every interior edge over the configured overlap and leans on the final division to make the last column and row
//! correct, where the move-a-tile-back rule makes the real overlap much larger. That works, but it makes the division
//! load-bearing for correctness rather than merely for tidiness. This project already answered the same question the
//! other way for the convolutional path, so using the real width makes each pair of abutting ramps a partition of
//! unity **before** the division and makes the two ways of combining tiles answer alike.
//!
//! # Two parity gaps the series leaves open
//!
//! **The noise field differs from the Go application's, so the same image at the same settings is not
//! pixel-identical between the two.** Go seeds with `math/rand/v2.NewChaCha8`, which is its own ChaCha8Rand
//! construction with a reseeding schedule of its own rather than the plain ChaCha8 stream `rand_chacha` produces, and
//! no Rust crate reproduces it. Every property the seed exists for is about determinism *within* one implementation
//! and is preserved in full — see [`noise`]. Nothing else in this project claims bit-parity either.
//!
//! **Nothing budgets, admits or refuses the memory one Osaka operation costs, and there is no warning.** Three
//! sessions are resident for the whole operation — a 6.8 GB transformer at FP16 plus 0.17 GB of VAE halves — and one
//! region measured **29.0 GB resident** through the CPU provider, the gap over the weights being activations with the
//! memory planner off. On top of that the image-scale buffers are float and scale with the **output**:
//!
//! ```text
//!   requested   output over a 12 MP photo   one planar float plane   peak float held
//!      1x                  12 MP                    0.14 GB              ~0.4 GB
//!      2x                  48 MP                    0.58 GB              ~1.7 GB
//!      4x                 192 MP                    2.30 GB              ~6.9 GB
//! ```
//!
//! Three planes are live at the peak — the resampled base, the accumulator's sum, and then either the weight plane or
//! the colour fix's scratch — which is why the accumulator resolves **in place** and the colour fix works in place on
//! that same buffer. The reference warns before a run it expects to exhaust the pool, reading a per-pool budget from
//! its model registry; this project has no memory budget at all, which [`sessions`](crate::sessions) already records
//! as a deferral, so there is nothing to read and a warning built on a guess is worse than none. The failure mode on
//! exhaustion is ONNX Runtime's allocator taking the process down. **The figures above are what the eventual budget
//! change starts from.**

pub(crate) mod colorfix;
pub(crate) mod graph;
pub(crate) mod latent;
#[cfg(test)]
mod live;
pub(crate) mod noise;
pub(crate) mod precision;
pub(crate) mod region;

use image::imageops::FilterType;
use image::{DynamicImage, ImageBuffer, Rgb};

use graph::{DiffusionVariant, Graph};
use imaging::canvas::{Canvas, placements};
use latent::{REGION_EDGE_PX, REGION_OVERLAP, padded_extent, target_size};
use region::{RegionError, RegionGraphs, RegionScratch, restore_region};

use crate::error::{InferenceError, UnsupportedReason};
use crate::models::precision::Precision;
use crate::models::{ArtifactId, GraphRole, ResolvedGraph, Scale};
use crate::pipeline::Backend;
use crate::pipeline::{ImagePipeline, Model, reporter};
use crate::providers::options::TRT_BUILDER_OPTIMIZATION_LEVEL;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile, ExecutionMode};
use crate::sessions::SessionHandle;
use imaging::grid::{Tile, TileGrid};
use imaging::tensor::{Channel, Normalisation, Sampler, chw_to_image, padded_to_chw};
use imaging::{ChannelDepth, TilingError};

/// What one diffusion upscale operation runs: its three graphs, and the scale the image is resampled to first.
///
/// Built by [`Upscale::pipeline`](crate::models::Upscale::pipeline) — the family's contract seam — which is the only
/// thing that names it. The chain driver holds it as an [`ImagePipeline`] and never learns that a diffusion contract
/// exists.
///
/// Every graph carries **its own** profile, which is where this parts company with the convolutional contract rather
/// than merely differing in count: the transformer's declaration is the one per-graph override in this project, and a
/// graph opened under another graph's configuration produces a working session and a wrong or slower result that
/// nothing downstream can detect. So the graphs travel whole — role, artifact and settings in one value — and
/// [`sessions`](Model::sessions) hands the pairs out already made.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Diffusion {
    /// The operation as a user would see it, for naming it in the failure this pipeline folds its own error into.
    name: String,
    /// The graphs this operation loads, in the order the variant declares them, each with its own artifact and its own
    /// profile. All three are opened together, since they are stages of one pass.
    graphs: Vec<ResolvedGraph>,
    /// The distinct artifacts this operation needs on disk, in the order it first needs them.
    required: Vec<ArtifactId>,
    /// The scale the image is resampled to before anything runs. The network is resolution-preserving, so this is
    /// what decides the size detail is restored at rather than something a pass overshoots.
    requested: Scale,
    /// Where the encoder, the transformer and the decoder sit in the order [`sessions`](Model::sessions) acquires
    /// them. Resolved once, here, rather than per run.
    ///
    /// By role rather than by position, because exchanging two of them compiles, runs, and returns a wrong image.
    roles: [usize; 3],
}

impl Diffusion {
    /// The pipeline running `graphs` for the operation named `name` at `requested`, with `required` on disk first.
    ///
    /// The role-to-position mapping is resolved here rather than per region, which is also where the refusal below
    /// belongs: a variant that does not declare all three roles cannot be run at all, and finding that out before the
    /// chain installs anything is the same guarantee every other refusal in the plan has.
    ///
    /// # Errors
    ///
    /// Returns [`InferenceError::Unsupported`] naming the operation where the graph set does not declare all three of
    /// the roles this pipeline addresses. Unreachable while every diffusion variant declares the three —
    /// `variant.rs` holds each to it — and written as a refusal rather than a panic, so that a variant added without
    /// one is a failed run rather than a crash.
    pub(crate) fn new(
        name: String,
        graphs: Vec<ResolvedGraph>,
        required: Vec<ArtifactId>,
        requested: Scale,
    ) -> Result<Self, InferenceError> {
        let position = |role| graphs.iter().position(|graph| graph.role == role);
        let roles = [GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder].map(position);

        let [Some(encoder), Some(transformer), Some(decoder)] = roles else {
            // Which role is missing is known exactly here — it is the arm of `roles` that came back `None` — so it
            // travels with the refusal rather than being flattened into "one of the three".
            let missing = [GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder]
                .into_iter()
                .zip(roles)
                .find_map(|(role, position)| position.is_none().then_some(role))
                .expect("a set that failed the destructure is missing at least one role");

            return Err(InferenceError::Unsupported {
                operation: name,
                reason: UnsupportedReason::IncompleteGraphSet { missing: missing.as_str() },
            });
        };

        Ok(Self { name, graphs, required, requested, roles: [encoder, transformer, decoder] })
    }
}

impl<B: Backend> Model<B> for Diffusion {
    fn required(&self) -> &[ArtifactId] {
        &self.required
    }

    /// Each graph's artifact against **its own** profile. See the type's own documentation for why one shared profile
    /// would be a wrong image rather than a tidier signature.
    fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)> {
        self.graphs.iter().map(|graph| (&graph.artifact, &graph.profile)).collect()
    }
}

impl<B: Backend> ImagePipeline<B> for Diffusion {
    /// One per graph: a region is one run of each rather than a sequence over them, and an image of many regions is
    /// still the same stages. Read off the declaration rather than restated, which is the move `PassSequence::stages`
    /// already makes with its own.
    fn stages(&self) -> usize {
        self.graphs.len()
    }

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError> {
        // Addressed by role rather than by position: the three were acquired in the variant's declared order, and
        // exchanging the encoder for the decoder compiles, runs, and returns a wrong image.
        let [encoder, transformer, decoder] = self.roles;
        let graphs = RegionGraphs::<B> {
            encoder: &sessions[encoder],
            transformer: &sessions[transformer],
            decoder: &sessions[decoder],
        };

        restore_image::<B>(input, &graphs, self.requested, depth, progress, cancelled)
            .map_err(|error| InferenceError::from_tiling(&self.name, error))
    }
}

/// What one Osaka run produces, or why there is no picture.
///
/// [`TilingError`] rather than an error of its own, so that [`InferenceError::from_tiling`](InferenceError)
/// needs no new arm and a failed Osaka run reads in a log exactly like a failed Kyoto one: `Untileable` for an image
/// with no area, `Cancelled` where the caller stopped it, `Tile` naming the region that failed.
type Restored<B> = Result<DynamicImage, TilingError<RegionError<<B as Backend>::Error>>>;

/// Runs `source` through the diffusion upscaler at `requested`, returning the result at `depth`.
///
/// `graphs` are the three sessions the chain acquired before any of them ran and holds for the whole call. They are
/// held across **every** region rather than across one, which makes the operation longer but not different in kind —
/// and is also why the memory the three graphs weigh does not come down partway through a run.
///
/// `progress` is reported over the whole operation, `0.0..=1.0`: the `0.0` at the start, one report per region, and
/// exactly `1.0` at the end. Per region is the finest granularity available — a region is tens of seconds of work —
/// and also the right one, since the bar then moves at a rate a user can read. `cancelled` is checked once per
/// region, on the same schedule.
///
/// # Errors
///
/// [`TilingError`], and in every case **no image at all** — not the regions that had already been restored, and not
/// the accumulator a cancellation landed in. See [`Restored`] for what each variant means here.
pub(crate) fn restore_image<B: Backend>(
    source: &DynamicImage,
    graphs: &RegionGraphs<'_, B>,
    requested: Scale,
    depth: ChannelDepth,
    progress: Option<&dyn Fn(f64)>,
    cancelled: &dyn Fn() -> bool,
) -> Restored<B> {
    // Dispatched once at the top, as `run_tiled` dispatches it, so the body below is written once and monomorphised
    // for `u8` and `u16` rather than branching per pixel over a nine-figure buffer.
    match depth {
        ChannelDepth::Eight => restored::<u8, B>(source, graphs, requested, progress, cancelled).map(u8::into_dynamic),
        ChannelDepth::Sixteen => {
            restored::<u16, B>(source, graphs, requested, progress, cancelled).map(u16::into_dynamic)
        }
    }
}

/// One run's picture at a known channel, or why there is none. [`Restored`] once the depth has been resolved.
type RestoredAt<T, B> = Result<ImageBuffer<Rgb<T>, Vec<T>>, TilingError<RegionError<<B as Backend>::Error>>>;

/// [`restore_image`]'s body, once, at whichever channel the caller asked for.
fn restored<T: Channel, B: Backend>(
    source: &DynamicImage,
    graphs: &RegionGraphs<'_, B>,
    requested: Scale,
    progress: Option<&dyn Fn(f64)>,
    cancelled: &dyn Fn() -> bool,
) -> RestoredAt<T, B>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    let (width, height) = (source.width(), source.height());

    // Before anything is allocated and before the division that computes a region's share of the progress. An
    // untouched buffer handed back with no error is indistinguishable from a finished enhancement.
    if width == 0 || height == 0 {
        return Err(TilingError::Untileable { width, height });
    }

    reporter(progress)(0.0);

    let (target_width, target_height) = target_size(width, height, requested);

    // The image the model is conditioned on and the reference the colour fix corrects towards. `resize_exact` rather
    // than `imageops::resize`, which goes through `DynamicImage`'s `GenericImageView` and is typed `Rgba<u8>` — that
    // would be where a 16-bit source silently became 8-bit, before the model had even seen it. Skipped outright where
    // the dimensions already match, which at a scale of 1 is seconds of filtering to produce the image it was given.
    let resampled;
    let base = if (target_width, target_height) == (width, height) {
        source
    } else {
        resampled = source.resize_exact(target_width, target_height, FilterType::Lanczos3);
        &resampled
    };

    let (padded_width, padded_height) = padded_extent(target_width, target_height);
    let sampler = Sampler::new(base);

    // Over the **padded** dimensions, which are at least one region on each axis and a multiple of 16 — so every
    // offset the grid produces is aligned and every region is exactly the one shape the graphs accept.
    let grid = TileGrid { size: REGION_EDGE_PX, overlap: REGION_OVERLAP, width: padded_width, height: padded_height };
    // Each region paired with what it shares with its neighbours, through the one helper the canvas tests drive too.
    let placed = placements(grid);

    if placed.is_empty() {
        return Err(TilingError::Untileable { width, height });
    }

    let mut canvas = Canvas::new(padded_width, padded_height);
    let mut scratch = RegionScratch::default();

    let step = 1.0 / placed.len() as f64;
    let mut reported = 0.0;

    for (index, (tile, shared)) in placed.iter().enumerate() {
        if cancelled() {
            return Err(TilingError::Cancelled);
        }

        // Clipped to the resampled image's own extent, so what `restore_region` is given is the part of the picture
        // this region actually covers; `image_to_chw` mirrors the rest out to 960 from the source itself. The mirror
        // is about the same last pixel either way, so this is arithmetically identical to cropping the region out of
        // a materialised padded copy rather than merely close to it.
        let covered = Tile {
            x: tile.x,
            y: tile.y,
            width: target_width.saturating_sub(tile.x).min(tile.width),
            height: target_height.saturating_sub(tile.y).min(tile.height),
        };

        let region = restore_region::<B>(graphs, &mut scratch, &sampler, covered)
            .map_err(|source| TilingError::Tile { index, source })?;

        canvas.add(region, *tile, *shared);

        if let Some(report) = progress {
            reported += step;

            // The accumulated sum of reciprocals does not land on 1, and a progress bar that stops at 99.98% is a bug
            // report. The last region reports exactly 1 rather than whatever the sum reached.
            report(if index + 1 == placed.len() { 1.0 } else { reported.min(1.0) });
        }
    }

    // In place: the accumulator *is* the result, and the weight plane goes with the canvas here rather than being
    // held alongside the colour fix's scratch.
    let mut assembled = canvas.resolve();

    // The one thing that genuinely needs the whole padded extent materialised, because the transform reads every
    // pixel of it: the reference the colour fix corrects towards. The region inputs do not — each is read straight
    // out of the source through the same reflection rule.
    //
    // Built here rather than before the region loop, which is the only place it is read. At a 4x output over a
    // 12-megapixel photograph it is 2.3 GB, and held across the loop it is 2.3 GB resident beside the canvas for the
    // whole run — the same peak the explicit drop below exists to bring down. Filling it costs a pass over the whole
    // extent too, which before the loop ran after `report(0.0)` and before the first region, so the bar sat at zero
    // through it.
    let mut conditioned = vec![0.0_f32; 3 * (padded_width as usize) * (padded_height as usize)];
    padded_to_chw(
        &mut conditioned,
        &sampler,
        (target_width, target_height),
        (padded_width, padded_height),
        Normalisation::Signed,
    )
    .expect("the base plane is allocated at the padded extent");

    // Once, on the assembled image, against the image the model was conditioned on — never per region. Before the
    // crop, because the transform replicates edge pixels outside the plane and a mirror of real picture content is
    // the better-conditioned input to a low pass than a replicated crop boundary would be.
    colorfix::wavelet_color_fix(&mut assembled, &conditioned, padded_width, padded_height)
        .expect("the canvas and the base plane are allocated at the same extent");

    // Nothing reads the base plane again, and at a 4x output over a 12-megapixel photograph it is 2.3 GB. Dropped
    // explicitly rather than at the end of the scope, so that the picture below is allocated against a peak that has
    // already come down — which is what keeps the pipeline holding three planes rather than four.
    drop(conditioned);

    // The one place a channel depth is applied, to the finished picture rather than to each region on its way in.
    let mut picture = ImageBuffer::<Rgb<T>, Vec<T>>::new(padded_width, padded_height);
    chw_to_image(&mut picture, &assembled, Normalisation::Signed)
        .expect("the picture is allocated at the padded extent");

    // For the same reason `conditioned` goes above, and at the same size: nothing reads the planar sum again, and the
    // crop below allocates a second whole image. Dropped before it rather than at the end of the scope, so the two
    // full-extent buffers are never resident together.
    drop(assembled);

    if (padded_width, padded_height) == (target_width, target_height) {
        return Ok(picture);
    }

    // The alignment extension off, leaving the dimensions the request asked for.
    Ok(image::imageops::crop_imm(&picture, 0, 0, target_width, target_height).to_image())
}

/// The ONNX Runtime graph transformers that miscompile Osaka's diffusion transformer.
///
/// Not a tuning choice. Under the pinned runtime they rewrite the graph so that it refers to a tensor they removed,
/// and session creation **on the CPU provider** then fails naming a node they created:
///
/// ```text
///   Attempting to get index by a name which does not exist:
///   InsertedPrecisionFreeCast_/dit/blocks.0/attn_norm/vid/Constant_output_0
///   for node: /dit/blocks.0/attn/norm_k/vid/Mul/SimplifiedLayerNormFusion/
/// ```
///
/// A runtime defect rather than an export one — neither name appears anywhere in the published `.onnx`, so both are
/// created at load time — and version-specific: the reference records the graph loading cleanly under ONNX Runtime
/// 1.29 and failing under the 1.26 pinned in [`crate::deps::artifact`]. It cannot be retired by re-exporting, only by
/// moving the bundled runtime forward, and it must be checked against *that* build rather than against whatever a
/// local Python install happens to have — which is how the reference briefly and wrongly declared it fixed.
///
/// The failure is also **CPU-specific**: on CoreML the graph opens either way, which follows from the node name, the
/// precision-free cast insertion being part of the CPU provider's FP16 handling. The setting is declared anyway,
/// because it is a session setting that reaches every provider and the CPU is the fallback every machine can land on.
///
/// # Two names, as the reference declares them
///
/// `SimplifiedLayerNormFusion` is the one measured to be sufficient: with it alone switched off, the graph opens.
/// `ReshapeFusion` is declared because the reference declares it and its comment describes both as miscompiling this
/// graph — an extra name costs one fusion this graph is demonstrably not relying on, and dropping it would mean
/// diverging from the reference on the strength of a measurement of a different question.
///
/// # What was wrong here before, because it is the kind of mistake this comment can prevent
///
/// This constant was a single name, and this comment asserted as measured fact that ONNX Runtime 1.26 takes
/// `optimization.disable_specified_optimizers` as one transformer *name* rather than as a list. **It does not.** The
/// sweep that concluded it joined the names with a comma, and the separator was hardcoded a layer below the thing
/// being swept — so every multi-name case in it was one unrecognised name, which the runtime ignores in silence, and
/// therefore failed exactly as naming nothing does. The table was consistent with a wrong separator and was read as a
/// runtime limitation. The separator is a semicolon; see [`DISABLED_OPTIMIZER_SEPARATOR`] and, for the measurement,
/// `models::upscale::osaka::live`.
///
/// An unrecognised name is ignored by the runtime rather than reported, so a typo here is silent and the only sign
/// the setting took effect is that the model loads at all. That is what made one wrong character survive three
/// changes, and it is the reason this setting is never taken on trust from documentation.
///
/// [`DISABLED_OPTIMIZER_SEPARATOR`]: crate::providers::profile::DISABLED_OPTIMIZER_SEPARATOR
const BROKEN_OPTIMIZERS: [&str; 2] = ["ReshapeFusion", "SimplifiedLayerNormFusion"];

/// Osaka: a VAE encoder, a one-step diffusion transformer and a VAE decoder, run as a single pass over the image at
/// its target size.
///
/// Unlike the convolutional variants, which hold one session per native scale, these three are stages of one pass and
/// are always loaded together — which is why the variant has no scale table.
pub(crate) static OSAKA: DiffusionVariant = DiffusionVariant {
    codename: "osaka",
    label: "Osaka",
    profile,
    graphs: &[
        Graph {
            role: GraphRole::Encoder,
            suffix: "_vae_encoder",
            pinned: Some(Precision::Fp16),
            cuda_prefer_nhwc: None,
        },
        // The one per-graph override in this project. 12,940 nodes of MatMul, Transpose and Softmax and not one
        // `Conv`, so there is no convolution kernel for NHWC to be faster at and what is left is the layout
        // transform's own cost: +1.3%, against -8.0% and -9.4% on the two halves either side of it.
        Graph { role: GraphRole::Transformer, suffix: "", pinned: None, cuda_prefer_nhwc: Some(false) },
        Graph {
            role: GraphRole::Decoder,
            suffix: "_vae_decoder",
            pinned: Some(Precision::Fp16),
            cuda_prefer_nhwc: None,
        },
    ],
};

/// The execution-provider tuning measured for this model, at the precision it carries.
///
/// Written at both precisions rather than split, because nothing in it is a property of the export's
/// types: the node count, the op mix and the runtime defect are the same graph either way — so the
/// precision is taken and deliberately unread, rather than dropped from the signature. A build that
/// published one of the two differently would name it here.
///
/// # The whole profile, transcribed rather than re-measured
///
/// Every percentage below is the reference implementation's, taken on hardware this change does not have, and
/// **none of them was re-measured here** — design.md D4. What was confirmed locally is that each setting reaches
/// the provider it names and that the model loads and runs with them; the percentages stay attributed.
///
/// The one exception is the disabled optimizers, which are correctness rather than tuning and **were** measured
/// here. That measurement confirmed the reference rather than correcting it, at the second attempt: the first
/// read a port defect as a runtime limitation and dropped one of the two names. See [`BROKEN_OPTIMIZERS`].
///
/// Written at both precisions rather than split, because nothing in it is a property of the export's types:
/// the node count, the op mix and the runtime defect are the same graph either way.
///
/// - **Sequential execution.** The transformer is 12,940 nodes — nearly five times Tokyo's 2,682 — arranged as
///   close to a linear backbone, so the inter-op pool is charged a handoff at every one of them and has no branch
///   wide enough to use it. -13.5% at FP16 and -10.1% at INT8 on the region, on all three graphs rather than one,
///   with output identical to the parallel run. On TensorRT it is a tie, that provider fusing each graph into a
///   single node.
/// - **The memory planner off.** Not for the usual reason: these shapes never vary, so the planner's own
///   assumption holds. It is simply a loss on activations this large — +22% on the VAE encoder with it on. On the
///   CUDA provider it is a tie on average and becomes bimodal once execution is sequential, half the passes at
///   381ms and half at 498-508ms, with individual runs as bad as 963ms on the decoder. A setting that is free on
///   average and occasionally costs 2.5x is not free.
/// - **`ReshapeFusion` and `SimplifiedLayerNormFusion` disabled.** Not tuning at all: without the second of them
///   the transformer does not open on the CPU provider. This is the one setting here that **was** verified
///   locally rather than transcribed, and it ends up exactly where the reference has it — both names, joined by
///   [`DISABLED_OPTIMIZER_SEPARATOR`], which is what has to be right for either to take effect. See
///   [`BROKEN_OPTIMIZERS`] for the failure it is declared against and for the reading of it that was wrong.
/// - **CoreML on the CPU and the GPU.** The default `ALL` lets CoreML dispatch to the Neural Engine, which these
///   graphs are consistently worse on: 4.2x on the VAE encoder and 3.9x on the decoder against `CPUAndGPU`, while
///   pinning the Neural Engine costs 2.5x on the transformer. The specialization strategy is deliberately left
///   alone — `FastPrediction` lands within noise on all three graphs and costs ~134s of session build on the
///   transformer.
/// - **TensorRT's builder optimization level at 3**, against the 5 every other model carries. The level is a
///   build-time knob here and nothing else: runtime is flat across 1, 3 and 5 — 140.9ms, 140.3ms and 142.6ms, one
///   spread of noise — while the engine build is 86s, 116s and 203s. It stops at 3, TensorRT's own default,
///   rather than at the 1 that measured the same, because level 0 is a cliff rather than a slope: it builds in 82s
///   and then runs the region in 533.7ms, worse than CUDA, the VAE decoder alone going from 45.1ms to 296.4ms.
///   Level 1 was fine on that card and is one step from that edge, on a sweep of a single GPU.
///
/// ## Why TensorRT's FP16 mode is refused rather than merely absent
///
/// Worth stating for each export, because they fail the same test for opposite reasons. On the **FP16** export
/// the flag is a no-op — TensorRT already honours the FP16 typing baked into the export — so it is absent there
/// because it buys nothing. On the **INT8** export it is not a no-op at all: that graph is FP32-typed apart from
/// its quantized weights, and the flag takes the transformer from 220.3ms to 78.4ms. It is refused anyway,
/// because that speed is bought with precision the caller did not ask for — the transformer drifts to a cosine
/// similarity of 0.9827 against the same graph on another provider, and the decoded region to 0.9954 with a max
/// absolute error of 2.0 on an image in `[-1,1]`, where every other configuration here stays at 0.9999. A user
/// who asked for INT8 must not be handed something further from the reference result than INT8 is. The honest
/// way to take that speed is to select the FP16 model, which is faster still and stays at 0.9999.
///
/// `trt_int8_enable` is refused for a second reason, and it is the one to remember before adding anything to
/// [`trt_options`](EpProfile::trt_options): **a profile applies to all three graphs.** The quantization is
/// weight-only `DequantizeLinear` with no Q on the activations, which TensorRT does not accelerate — the
/// transformer measures as noise with it — but the flag reaches the two VAE halves too, and there it is a
/// catastrophe: the encoder goes from 21.4ms to 124.6ms and the decoder from 45.5ms to 298.9ms. Neither mode is
/// named here, and both are already off in [`crate::providers::options`]'s pinned defaults; this arm's job is to
/// record that the absence is a decision.
///
/// ## What the CUDA layout setting does, and where it is taken back off
///
/// [`cuda_prefer_nhwc`](EpProfile::cuda_prefer_nhwc) is declared here for the variant and **overridden off for
/// the transformer alone**, where the graph set is resolved — see [`graph`]. The two VAE halves are 27
/// and 38 convolutions at FP16, which is the shape cuDNN has tensor-core kernels for in NHWC and which otherwise
/// pays for a transpose on either side of every convolution: -8.0% on the encoder and -9.4% on the decoder, at
/// cosine 1.00000 on all three graphs. The transformer contains **no convolution whatsoever** — 12,940 nodes of
/// MatMul, Transpose and Softmax and not one `Conv` — so there is nothing for an NHWC kernel to be faster at and
/// what is left is the layout transform's own cost, measured at +1.3%.
///
/// ## What was measured and rejected
///
/// Recorded so the next reader with the hardware does not re-run them. Nothing in the CUDA option set moves these
/// graphs — every one of them is already at its best value in this project's pinned defaults. `use_tf32` matters
/// more than everything else put together and is already on: turning it off takes the decoder from 132.9ms to
/// 23.565s. `sdpa_kernel` does nothing at either setting, because this transformer's attention is exported as 400
/// loose `Softmax` nodes that nothing in the 1.26 fusion pipeline puts back together. On TensorRT, a 24 GB
/// workspace against the default 4 GB, `trt_auxiliary_streams` at 1 and at 4, and `trt_layer_norm_fp32_fallback`
/// are all ties — the last one TensorRT itself suggests in a build warning, and taking that suggestion costs 1.3%
/// and moves the result further from CUDA rather than nearer.
///
/// [`DISABLED_OPTIMIZER_SEPARATOR`]: crate::providers::profile::DISABLED_OPTIMIZER_SEPARATOR
pub(crate) fn profile(_precision: Precision) -> EpProfile {
    // Both precisions, and every figure the reference's. The transformer takes `cuda_prefer_nhwc` back off
    // where the graph set is resolved, so the override is a property of that graph rather than something a
    // caller remembers to apply.
    EpProfile {
        coreml_compute_units: CoreMlComputeUnits::CpuAndGpu,
        execution_mode: ExecutionMode::Sequential,
        disable_mem_pattern: true,
        disabled_optimizers: BROKEN_OPTIMIZERS.iter().map(|name| (*name).to_string()).collect(),
        cuda_prefer_nhwc: true,
        trt_options: [(TRT_BUILDER_OPTIMIZATION_LEVEL.to_string(), "3".to_string())].into(),
        // Declined for all three graphs, and not for speed. The WebGPU plugin uploads a graph's weights into GPU
        // buffers, and on the integrated GPUs it exists to serve those buffers are system RAM the driver pins outside
        // any process's accounting — so the transformer's 3.7 GB of INT8 weights become 3.7 GB the kernel cannot
        // reclaim, beside the copy ONNX Runtime already holds and the two VAEs. On a 16 GB laptop that was not an
        // out-of-memory error in the application but the whole machine locking up. The other models are a few hundred
        // megabytes at most and never approach it.
        webgpu_declined: true,
        ..EpProfile::default()
    }
}

#[cfg(test)]
mod tests {
    use crate::pipeline::test_support::stub_backend_runs;
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex};

    use image::GenericImageView as _;

    use super::latent::REGION_EDGE;
    use super::*;
    use crate::error::SessionError;
    use crate::models::ArtifactId;
    use crate::pipeline::session::GraphShape;
    use crate::providers::ExecutionProvider;
    use crate::providers::profile::EpProfile;
    use crate::sessions::SessionHandle;
    use imaging::canvas::axis_weights;
    use imaging::canvas::fixtures::flat;
    use imaging::canvas::placements;
    use imaging::test_support::gradient;

    /// What the fake recorded: the two shapes of every graph call, in order.
    type Shapes = Arc<Mutex<Vec<(&'static str, GraphShape, GraphShape)>>>;

    /// A stand-in session: which of the three roles it fills, and what the whole set was asked to do.
    struct FakeSession {
        role: &'static str,
        shapes: Shapes,
        /// What every decoded value is, so a result that is not this came from somewhere else.
        level: f32,
        /// The region index whose decode refuses, counting from zero.
        fails_at: Option<usize>,
    }

    /// A backend whose three graphs record what they were asked for and hand back a value of the driver's choosing.
    struct Fake;

    impl Backend for Fake {
        type Session = FakeSession;
        type Error = std::io::Error;

        async fn acquire(
            &self,
            _artifact: &ArtifactId,
            _profile: &EpProfile,
            _requested: ExecutionProvider,
            _interest: &crate::sessions::Interest,
        ) -> Result<SessionHandle<Self::Session>, SessionError> {
            unreachable!("the driver is handed sessions the chain already acquired")
        }

        stub_backend_runs!("a region takes the shaped seam, never the tiled one"; run_tile);

        fn run_graph(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            output: &mut [f32],
            output_shape: GraphShape,
        ) -> Result<(), Self::Error> {
            let session = handle.session();

            let decodes = {
                let mut shapes = session.shapes.lock().unwrap();
                shapes.push((session.role, input_shape, output_shape));
                shapes.iter().filter(|(role, _, _)| *role == "decoder").count() - usize::from(session.role == "decoder")
            };

            if session.role == "decoder" && session.fails_at == Some(decodes) {
                return Err(std::io::Error::other("the decoder refused"));
            }

            // Refused exactly as the real seam does: a buffer the declared shape does not describe is the mistake the
            // seam exists to prevent.
            assert_eq!(
                input.len(),
                input_shape.len(),
                "{} was fed a buffer {input_shape} does not describe",
                session.role
            );
            assert_eq!(output.len(), output_shape.len(), "{}'s output buffer is not {output_shape}", session.role);

            // The decoder answers with the level this run was built for; the two latent stages answer with something
            // derived from what arrived, so a stage that was skipped does not produce what a correct sequence would.
            if session.role == "decoder" {
                output.fill(session.level);
            } else {
                let mean = input.iter().sum::<f32>() / input.len() as f32;
                output.fill(mean.clamp(-1.0, 1.0));
            }

            Ok(())
        }

        // The two seams a diffusion suite never reaches — see `stub_backend_runs`.
        stub_backend_runs!(
            "nothing here runs a graph with more than one output, or one fed a value beside its image";
            run_named_outputs,
            run_weighted,
        );
    }

    /// The three handles for one run, recording into `shapes`.
    fn held(shapes: &Shapes, level: f32, fails_at: Option<usize>) -> [SessionHandle<FakeSession>; 3] {
        ["encoder", "transformer", "decoder"].map(|role| {
            let session = FakeSession { role, shapes: Arc::clone(shapes), level, fails_at };
            SessionHandle::held(session, ExecutionProvider::Cpu)
        })
    }

    /// A scale, stated as the number a user asked for.
    fn scale(factor: f64) -> Scale {
        Scale::new(factor).unwrap_or_else(|error| panic!("{factor} is in range: {error}"))
    }

    /// One run of the whole pipeline, with nothing watching it, and the graph calls it made.
    fn run(source: &DynamicImage, factor: f64, depth: ChannelDepth) -> (DynamicImage, Shapes) {
        let shapes: Shapes = Arc::default();
        let handles = held(&shapes, 0.0, None);
        let graphs = RegionGraphs::<Fake> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };

        let produced = restore_image::<Fake>(source, &graphs, scale(factor), depth, None, &|| false)
            .unwrap_or_else(|error| panic!("the pipeline refused a {factor}x run: {error}"));

        (produced, shapes)
    }

    /// The shapes the published FP16 exports declare, which every region must arrive at whatever the image was.
    const PIXELS: GraphShape = GraphShape::new(3, REGION_EDGE, REGION_EDGE);
    const LATENT: GraphShape = GraphShape::new(16, REGION_EDGE / 8, REGION_EDGE / 8);
    const PACKED: GraphShape = GraphShape::new(33, REGION_EDGE / 8, REGION_EDGE / 8);

    #[test]
    fn every_pixel_of_the_padded_extent_is_covered_and_no_seam_shows() {
        // A flat source and a flat decode, over an extent wide enough to need two regions whose shared width is the
        // one the grid actually produced rather than the configured 128. Every pixel of the result is then the
        // source's own colour: a pixel no region covered would resolve to a zero weight and come back mid-grey, and a
        // seam would come back as a band. The colour fix is what turns the flat decode back into the source's
        // colour — which is itself the thing it exists to do.
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1000, 500, Rgb([64, 160, 200])));
        let (produced, shapes) = run(&source, 1.0, ChannelDepth::Eight);

        assert_eq!(shapes.lock().unwrap().len(), 6, "1000x500 padded to 1008x960 is two regions, not one");
        assert_eq!(produced.dimensions(), (1000, 500));

        let pixels = produced.to_rgb8();
        for (x, y, pixel) in pixels.enumerate_pixels() {
            for channel in 0..3 {
                let expected = [64_u8, 160, 200][channel];
                assert!(
                    pixel.0[channel].abs_diff(expected) <= 1,
                    "({x}, {y}) came back as {:?} rather than the source's own colour",
                    pixel.0
                );
            }
        }
    }

    #[test]
    fn every_region_arrives_at_exactly_the_one_size_the_graphs_accept() {
        // Whatever the image was: an image smaller than a region is mirrored up to one, and an image that is not a
        // multiple of 16 is extended to one. The tensor is always 960x960 either way.
        for (width, height, factor, regions) in
            [(64, 64, 1.0, 1), (17, 900, 1.0, 1), (1000, 500, 1.0, 2), (700, 200, 2.0, 2)]
        {
            let (_, shapes) = run(&gradient(width, height), factor, ChannelDepth::Eight);
            let calls = shapes.lock().unwrap().clone();

            assert_eq!(calls.len(), 3 * regions, "{width}x{height} at {factor}x did not run {regions} region(s)");

            for chunk in calls.chunks(3) {
                assert_eq!(chunk[0], ("encoder", PIXELS, LATENT), "{width}x{height} at {factor}x");
                assert_eq!(chunk[1], ("transformer", PACKED, LATENT), "{width}x{height} at {factor}x");
                assert_eq!(chunk[2], ("decoder", LATENT, PIXELS), "{width}x{height} at {factor}x");
            }
        }
    }

    #[test]
    fn an_image_with_no_area_is_refused_before_anything_runs() {
        for (width, height) in [(0, 100), (100, 0), (0, 0)] {
            let shapes: Shapes = Arc::default();
            let handles = held(&shapes, 0.0, None);
            let graphs = RegionGraphs::<Fake> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };

            let source = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));
            let outcome = restore_image::<Fake>(&source, &graphs, scale(2.0), ChannelDepth::Eight, None, &|| false);

            assert!(
                matches!(outcome, Err(TilingError::Untileable { width: w, height: h }) if (w, h) == (width, height)),
                "a {width}x{height} image was not refused"
            );
            assert!(shapes.lock().unwrap().is_empty(), "a graph ran for an image with no area");
        }
    }

    #[test]
    fn the_result_is_the_sources_dimensions_multiplied_by_the_requested_scale_and_rounded() {
        // Including a scale of 1, which is a restoration rather than a no-op, and a scale no whole factor serves —
        // the arithmetic is the same one the convolutional correction produces, so which contract served a request
        // is not observable in the size of what comes back.
        for (width, height, factor, expected) in [
            (64, 64, 1.0, (64, 64)),
            (301, 201, 1.5, (452, 302)),
            (200, 150, 2.0, (400, 300)),
            (63, 41, 7.0, (441, 287)),
        ] {
            let (produced, _) = run(&gradient(width, height), factor, ChannelDepth::Eight);

            assert_eq!(
                produced.dimensions(),
                expected,
                "{width}x{height} at {factor}x came back at the wrong size, so the alignment extension survived"
            );
        }
    }

    #[test]
    fn a_sixteen_bit_request_comes_back_at_sixteen_bits() {
        // The crop is the last thing that touches the pixels, and going through `DynamicImage`'s own
        // `GenericImageView` there would narrow the result to `Rgba<u8>` on the final line of the pipeline.
        let (eight, _) = run(&gradient(64, 64), 1.0, ChannelDepth::Eight);
        let (sixteen, _) = run(&gradient(64, 64), 1.0, ChannelDepth::Sixteen);

        assert!(matches!(eight, DynamicImage::ImageRgb8(_)), "an 8-bit request came back as {eight:?}");
        assert!(matches!(sixteen, DynamicImage::ImageRgb16(_)), "a 16-bit request came back as {sixteen:?}");

        // And at a size that needed cropping, which is the line the narrowing would have happened on.
        let (cropped, _) = run(&gradient(301, 201), 1.5, ChannelDepth::Sixteen);
        assert!(matches!(cropped, DynamicImage::ImageRgb16(_)), "the crop narrowed the result to {cropped:?}");
    }

    #[test]
    fn the_reported_sequence_opens_on_zero_never_goes_backwards_and_lands_on_exactly_one() {
        let reported = RefCell::new(Vec::new());
        let shapes: Shapes = Arc::default();
        let handles = held(&shapes, 0.0, None);
        let graphs = RegionGraphs::<Fake> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };

        restore_image::<Fake>(
            &gradient(1000, 500),
            &graphs,
            scale(1.0),
            ChannelDepth::Eight,
            Some(&|fraction| reported.borrow_mut().push(fraction)),
            &|| false,
        )
        .expect("the pipeline restores");

        let reported = reported.into_inner();

        // The `0.0` at the start and one report per region, which is the finest granularity there is: a region is
        // tens of seconds of work.
        assert_eq!(reported.len(), 3, "two regions did not report a zero and one fraction each: {reported:?}");
        assert_eq!(reported.first().copied(), Some(0.0), "the run did not open on zero");
        assert_eq!(reported.last().copied(), Some(1.0), "the run did not land on exactly one");

        for pair in reported.windows(2) {
            assert!(pair[1] >= pair[0], "the bar went backwards: {} then {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn a_cancellation_partway_through_produces_no_image_at_all() {
        // Not the regions that had already been restored, and not the accumulator they were added to: a canvas with
        // half its regions in it is still an image, and nothing downstream could tell it from a finished one.
        let shapes: Shapes = Arc::default();
        let handles = held(&shapes, 0.0, None);
        let graphs = RegionGraphs::<Fake> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };

        let stop_after_one = || shapes.lock().unwrap().len() >= 3;

        let outcome = restore_image::<Fake>(
            &gradient(1000, 500),
            &graphs,
            scale(1.0),
            ChannelDepth::Eight,
            None,
            &stop_after_one,
        );

        assert!(matches!(outcome, Err(TilingError::Cancelled)), "a cancelled run produced {outcome:?}");
        assert_eq!(shapes.lock().unwrap().len(), 3, "the run continued past the cancellation");
    }

    #[test]
    fn a_failing_region_names_its_index_and_produces_no_partial_picture() {
        for failing in [0_usize, 1] {
            let shapes: Shapes = Arc::default();
            let handles = held(&shapes, 0.0, Some(failing));
            let graphs = RegionGraphs::<Fake> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };

            let outcome =
                restore_image::<Fake>(&gradient(1000, 500), &graphs, scale(1.0), ChannelDepth::Eight, None, &|| false);

            let Err(TilingError::Tile { index, source }) = outcome else {
                panic!("a failing region produced {outcome:?}");
            };

            assert_eq!(index, failing, "the failure named the wrong region");
            assert!(
                matches!(source, RegionError::Graph { role: "decoder", .. }),
                "the failure did not carry the graph's own error: {source}"
            );

            // It stopped there rather than running on, so no later region was added to a canvas nobody will resolve.
            assert_eq!(shapes.lock().unwrap().len(), 3 * (failing + 1), "the run continued past the failing region");
        }
    }

    /// A graph filling `role`, for the tests about which roles a set declares rather than about what they compute.
    fn graph(role: GraphRole) -> ResolvedGraph {
        ResolvedGraph {
            role,
            artifact: ArtifactId::new(crate::models::Family::Upscale, "osaka", None, Precision::Fp16),
            profile: EpProfile::default(),
        }
    }

    #[test]
    fn a_graph_set_that_does_not_declare_all_three_roles_is_refused_rather_than_panicking() {
        // The refusal `Work::of` used to make on the chain's side, now made where the roles are resolved — and made
        // once, before anything is installed, rather than per region. Unreachable while every diffusion variant
        // declares the three; what it buys is that a variant added without one is a failed run naming itself rather
        // than an index out of bounds in the middle of an image.
        for missing in [GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder] {
            let declared = [GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder]
                .into_iter()
                .filter(|role| *role != missing)
                .map(graph)
                .collect();

            let outcome = Diffusion::new(
                "Osaka (FP16)".to_string(),
                declared,
                Vec::new(),
                Scale::new(2.0).expect("the test supplied a scale in range"),
            );

            let Err(InferenceError::Unsupported { operation, .. }) = outcome else {
                panic!("a set declaring no {missing} was accepted");
            };

            // Named as a user would see it, so a front end can say which enhancement it cannot run.
            assert_eq!(operation, "Osaka (FP16)");
        }
    }

    #[test]
    fn a_graph_set_declaring_the_three_roles_out_of_order_still_addresses_each_by_role() {
        // The positions are resolved from the roles rather than assumed from the order, which is what keeps a variant
        // that lists its decoder first from running its pixels through the decoder on the way in.
        let reversed = vec![graph(GraphRole::Decoder), graph(GraphRole::Transformer), graph(GraphRole::Encoder)];

        let pipeline = Diffusion::new(
            "Osaka (FP16)".to_string(),
            reversed,
            Vec::new(),
            Scale::new(2.0).expect("the test supplied a scale in range"),
        )
        .expect("a set declaring all three roles");

        assert_eq!(pipeline.roles, [2, 1, 0], "the roles were taken from their positions rather than their names");
    }

    // The weighted accumulator, driven at this model's own region geometry. `canvas` itself is cross-family and its
    // tests there are written against synthetic extents; these two are the ones that assert it normalises over the
    // 960-pixel regions and the 128-pixel overlap step 4 above actually hands it, so they live with the geometry.

    #[test]
    fn a_uniform_image_tiled_at_osakas_geometry_resolves_back_to_that_value_everywhere() {
        // At the real geometry, over an extent wide enough to need three columns — so the last column is one the grid
        // moved back, sharing 944 pixels rather than the 128 that was configured. The property is that a uniform
        // picture comes back uniform whatever the weights were, which is what "the weights sum to what they divide
        // by" means when it is checked rather than argued.
        let (width, height) = (2 * REGION_EDGE_PX - REGION_OVERLAP + 16, REGION_EDGE_PX);
        let grid = TileGrid { size: REGION_EDGE_PX, overlap: REGION_OVERLAP, width, height };

        assert!(grid.layout().columns() > 1, "the extent chosen does not actually tile");

        let mut canvas = Canvas::new(width, height);
        for (tile, shared) in placements(grid) {
            canvas.add(&flat(tile.width, tile.height, 0.25), tile, shared);
        }

        let resolved = canvas.resolve();
        assert_eq!(resolved.len(), 3 * (width as usize) * (height as usize));

        for (index, value) in resolved.iter().enumerate() {
            assert!(
                (value - 0.25).abs() < 1e-5,
                "the uniform value came back as {value} at {index}, so the weights did not normalise"
            );
        }
    }

    #[test]
    fn two_abutting_ramps_over_the_width_the_regions_actually_share_sum_to_one() {
        // D4's own property, which is what makes the division tidy rather than load-bearing. Checked at the real
        // overlap and at a wide one the grid produces down the last column.
        for width in [1, 2, REGION_OVERLAP, 944] {
            let left = axis_weights(960, 0, width);
            let right = axis_weights(960, width, 0);

            for offset in 0..width as usize {
                let falling = left[960 - width as usize + offset];
                let rising = right[offset];

                assert!(
                    (falling + rising - 1.0).abs() < 1e-6,
                    "a {width}-pixel seam sums to {} at {offset}",
                    falling + rising
                );
            }
        }
    }
}
