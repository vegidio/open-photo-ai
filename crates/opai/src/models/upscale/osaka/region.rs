//! One region through the three graphs: encode, take one diffusion step, decode.
//!
//! The innermost step of the pipeline. Everything around it — the Lanczos resample to the target size, the tile
//! grid, the weighted canvas and the wavelet colour fix — lives in [`super`].
//!
//! # The three sessions are held across the whole region
//!
//! Unchanged in kind from the convolutional path, wider in degree: the caller acquires every session before any of
//! them runs and holds each handle for the whole call. Holding the handle **is** the guarantee — the session cache's
//! "in use" test is the handle's own reference count, so a model a run holds cannot be reclaimed by the idle sweep, by
//! an explicit release, or by a provider switch. For a region that is three handles for one operation rather than one
//! per pass, and there is no window in which one could be reclaimed mid-region.
//!
//! The three may be built on **different providers**, because the CPU fallback is per session: a machine where the
//! transformer fails to build on CoreML but the VAE halves succeed runs the region across two. That is correct and
//! already reported, and it is worth knowing because it is a plausible explanation for a region slower than either
//! provider alone.
//!
//! # A failure produces no region at all
//!
//! Not the stages that had already run, and not a half-written buffer. Half a restored latent beside the previous
//! region's tail is still an image, and nothing downstream could tell it from a finished one.

use super::latent::{
    LATENT_CHANNELS, REGION_EDGE, REGION_EDGE_PX, TRANSFORMER_CHANNELS, VAE_STRIDE, pack, scheduler_step,
};
use super::noise::{NOISE_SEED, gaussian};
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::sessions::SessionHandle;
use imaging::grid::Tile;
use imaging::tensor::{Normalisation, Sampler, TensorShape, image_to_chw};

/// The latent edge one region compresses to.
const LATENT_EDGE: usize = REGION_EDGE / VAE_STRIDE;

/// The three sessions one region runs, already acquired and addressed by what they do.
///
/// A struct rather than three parameters, and named by role rather than ordered, for the same reason the resolved
/// graph set is: two handles of the same type in a row is an argument order a caller can get wrong, and exchanging
/// the encoder for the decoder compiles, runs, and returns a wrong image.
pub(crate) struct RegionGraphs<'a, B: Backend> {
    /// Turns the region's pixels into the latent the transformer works on.
    pub(crate) encoder: &'a SessionHandle<B::Session>,
    /// Holds nearly all of the variant's weight and does the restoration.
    pub(crate) transformer: &'a SessionHandle<B::Session>,
    /// Turns the restored latent back into pixels.
    pub(crate) decoder: &'a SessionHandle<B::Session>,
}

/// Why one region could not be restored.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RegionError<E> {
    /// The region's extent is not one the transformer's single region size can be filled from.
    ///
    /// The tensor is always exactly [`REGION_EDGE`] square — the published exports carry no symbolic dimension on any
    /// axis, so nothing else runs — but the *extent* handed in is the part of the image the region actually covers,
    /// which the driver clips to the resampled image and which [`image_to_chw`] mirrors out to fill the rest. So what
    /// is refused here is an extent that could not fill one: an empty one, with no pixel for the mirror to come from,
    /// and one larger than a region, which would silently drop the pixels past its edge.
    ///
    /// Checked here rather than left to the runtime, which would report the first as an assertion from inside the pad
    /// and the second not at all.
    #[error("the region covers {width}x{height}, which cannot fill the {REGION_EDGE}x{REGION_EDGE} the model accepts")]
    WrongSize {
        /// The width that was asked for.
        width: u32,
        /// The height that was asked for.
        height: u32,
    },

    /// A buffer handed to one of the two conversions was not the length its shape requires.
    #[error(transparent)]
    Shape(#[from] TensorShape),

    /// One of the three graphs failed. The role is carried because which one failed is the difference between a
    /// model that cannot run at all and something about this region.
    #[error("the {role} failed on this region")]
    Graph {
        /// Which of the three it was.
        role: &'static str,
        /// What the backend reported.
        #[source]
        source: E,
    },
}

/// The scratch one region loop reuses, rather than seven identically-shaped allocations per region.
///
/// Every region is exactly [`REGION_EDGE`] square — the transformer accepts nothing else — so every buffer here is
/// the same length from one region to the next. Allocating per region would mean hundreds of megabytes of garbage
/// over a pass, on a machine already holding a 7 GB model.
///
/// Reuse is safe for the reason it is there: each buffer is fully overwritten before it is read again.
pub(crate) struct RegionScratch {
    /// The region's pixels, planar, in `[-1, 1]`.
    pixels: Vec<f32>,
    /// What the encoder produced: the condition latent.
    condition: Vec<f32>,
    /// This region's noise field.
    noise: Vec<f32>,
    /// The transformer's 33-channel input.
    packed: Vec<f32>,
    /// What the transformer produced, which is the flow-matching velocity rather than a latent.
    prediction: Vec<f32>,
    /// The clean latent the scheduler step derives, which is what the decoder is given.
    denoised: Vec<f32>,
    /// What the decoder produced, planar, in `[-1, 1]`.
    restored: Vec<f32>,
}

impl Default for RegionScratch {
    /// Every buffer at the length the shape it is fed to, or read back from, declares — rather than at an arithmetic
    /// of its own that could drift from it. A scratch buffer that disagrees with its graph's shape is what
    /// [`GraphShape`] exists to make visible, so composing it from the same constant is the cheapest way to keep them
    /// from ever disagreeing.
    fn default() -> Self {
        Self {
            pixels: vec![0.0; PIXELS.len()],
            condition: vec![0.0; LATENT.len()],
            noise: vec![0.0; LATENT.len()],
            packed: vec![0.0; PACKED.len()],
            prediction: vec![0.0; LATENT.len()],
            denoised: vec![0.0; LATENT.len()],
            restored: vec![0.0; PIXELS.len()],
        }
    }
}

/// The shapes the three graphs are run at, which the published exports declare and this slice confirmed.
const PIXELS: GraphShape = GraphShape::new(3, REGION_EDGE, REGION_EDGE);
const LATENT: GraphShape = GraphShape::new(LATENT_CHANNELS, LATENT_EDGE, LATENT_EDGE);
const PACKED: GraphShape = GraphShape::new(TRANSFORMER_CHANNELS, LATENT_EDGE, LATENT_EDGE);

/// What one region restores to: the decoder's own planar output, or why there is none.
///
/// A named result rather than the `Result<&[f32], RegionError<..>>` spelled out, which carries a lifetime and a
/// generic parameter two deep and is unreadable at every call site.
type Restored<'a, B> = Result<&'a [f32], RegionError<<B as Backend>::Error>>;

/// Restores `region` of `source` and hands back the decoder's planar output, `[-1, 1]`, three planes of
/// [`REGION_EDGE`] square.
///
/// The model preserves resolution: what comes back is the region at its own size, restored rather than enlarged.
///
/// **Planar float rather than an image at a channel depth**, because the accumulator this feeds averages every
/// contribution and divides once — and quantising a stochastic region on its way into that average throws away
/// precision exactly where the averaging needs it. The depth is applied once, to the finished picture.
///
/// `region` is the part of the image this region actually covers, which the driver has already clipped to the
/// resampled image's own extent. Where it is short of [`REGION_EDGE`] on either axis — the alignment extension, or a
/// whole image smaller than one region — [`image_to_chw`] mirrors the region's own edge pixels out to fill the rest,
/// so the tensor is always the one shape the graphs accept and nothing is materialised as a padded copy.
///
/// `region`'s origin is what the noise field is keyed on, so the same region of the same image always draws the same
/// noise however many regions preceded it — see [`noise`](super::noise).
///
/// The pixels go in at [`Normalisation::Signed`], the `[-1, 1]` range this pipeline is the first caller of. Every
/// convolutional model here was trained against `[0, 1]`; feeding this one that range is not an error the runtime can
/// report, it is a worse image.
///
/// # Errors
///
/// [`RegionError`], and **no region at all** in every case — not the stages that had already run. The borrow is what
/// makes that structural: a failure returns the error rather than the scratch, so there is no way to read the buffer
/// a failed stage left behind.
pub(crate) fn restore_region<'a, B: Backend>(
    graphs: &RegionGraphs<'_, B>,
    scratch: &'a mut RegionScratch,
    source: &Sampler<'_>,
    region: Tile,
) -> Restored<'a, B> {
    if region.width == 0 || region.height == 0 || region.width > REGION_EDGE_PX || region.height > REGION_EDGE_PX {
        return Err(RegionError::WrongSize { width: region.width, height: region.height });
    }

    image_to_chw(&mut scratch.pixels, source, region, REGION_EDGE_PX, Normalisation::Signed)?;

    // The three graphs in order, each under its own short lock. The region is three locks rather than one held
    // across all of it, which is what keeps an export queue's second image interleaving with the first.
    B::run_graph(graphs.encoder, &scratch.pixels, PIXELS, &mut scratch.condition, LATENT)
        .map_err(|source| RegionError::Graph { role: "encoder", source })?;

    gaussian(&mut scratch.noise, region.x, region.y, NOISE_SEED);
    pack(&mut scratch.packed, &scratch.condition, &scratch.noise, LATENT_EDGE * LATENT_EDGE);

    B::run_graph(graphs.transformer, &scratch.packed, PACKED, &mut scratch.prediction, LATENT)
        .map_err(|source| RegionError::Graph { role: "transformer", source })?;

    // What the transformer returned is the velocity, not the latent, whatever the output tensor is named. Skipping
    // this hands the decoder the image buried under the velocity field — which still decodes, and scores worse than
    // the input it was given. See `latent::scheduler_step`.
    scheduler_step(&mut scratch.denoised, &scratch.prediction, &scratch.noise);

    B::run_graph(graphs.decoder, &scratch.denoised, LATENT, &mut scratch.restored, PIXELS)
        .map_err(|source| RegionError::Graph { role: "decoder", source })?;

    Ok(&scratch.restored)
}

#[cfg(test)]
mod tests {
    use crate::pipeline::test_support::stub_backend_runs;
    use std::sync::{Arc, Mutex};

    use image::DynamicImage;

    use super::*;
    use crate::error::SessionError;
    use crate::models::ArtifactId;
    use crate::providers::ExecutionProvider;
    use crate::providers::profile::EpProfile;

    /// What the fake recorded: the graph each call was made against and the two shapes it was declared with.
    type Calls = Arc<Mutex<Vec<(&'static str, GraphShape, GraphShape)>>>;

    /// A stand-in session: which of the three roles it fills, and whether it refuses.
    struct FakeSession {
        role: &'static str,
        calls: Calls,
        fails: bool,
    }

    /// A backend whose three graphs record what they were asked for and answer with a value derived from what
    /// arrived, so a stage that was skipped or run out of order does not produce what a correct sequence would.
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
            unreachable!("the region is handed sessions the chain already acquired")
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
            session.calls.lock().unwrap().push((session.role, input_shape, output_shape));

            if session.fails {
                return Err(std::io::Error::other(format!("the {} refused", session.role)));
            }

            // The buffers the caller supplied must already match what it declared, which is the mistake this seam
            // exists to prevent — so the fake refuses it exactly as the real one does.
            assert_eq!(
                input.len(),
                input_shape.len(),
                "{} was fed a buffer {input_shape} does not describe",
                session.role
            );
            assert_eq!(output.len(), output_shape.len(), "{}'s output buffer is not {output_shape}", session.role);

            // Derived from the input rather than constant, so a stage fed the wrong buffer produces a different
            // answer than one fed the right buffer.
            let mean = input.iter().sum::<f32>() / input.len() as f32;
            output.fill(mean.clamp(-1.0, 1.0));

            Ok(())
        }

        // The two seams a diffusion suite never reaches — see `stub_backend_runs`.
        stub_backend_runs!(
            "nothing here runs a graph with more than one output, or one fed a value beside its image";
            run_named_outputs,
            run_weighted,
        );
    }

    /// The three handles for one region, each recording into `calls`, with `failing` refusing if named.
    fn graphs(calls: &Calls, failing: Option<&'static str>) -> [SessionHandle<FakeSession>; 3] {
        ["encoder", "transformer", "decoder"].map(|role| {
            let session = FakeSession { role, calls: Arc::clone(calls), fails: failing == Some(role) };
            SessionHandle::held(session, ExecutionProvider::Cpu)
        })
    }

    /// One 960x960 region of a source large enough to hold it.
    fn region() -> Tile {
        Tile { x: 0, y: 0, width: REGION_EDGE_PX, height: REGION_EDGE_PX }
    }

    /// A source exactly one region across.
    fn source() -> DynamicImage {
        imaging::test_support::gradient(REGION_EDGE_PX, REGION_EDGE_PX)
    }

    #[test]
    fn the_three_graphs_are_called_in_order_with_the_shapes_the_exports_declare() {
        // The shapes are the published FP16 exports': `[1,3,960,960]` in and `[1,16,120,120]` out of the encoder,
        // `[1,33,120,120]` in and `[1,16,120,120]` out of the transformer, and the decoder the encoder's inverse.
        let calls: Calls = Arc::default();
        let held = graphs(&calls, None);
        let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };

        let source = source();
        let mut scratch = RegionScratch::default();
        let restored = restore_region::<Fake>(&set, &mut scratch, &Sampler::new(&source), region())
            .expect("the fake restores a region");

        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                ("encoder", GraphShape::new(3, 960, 960), GraphShape::new(16, 120, 120)),
                ("transformer", GraphShape::new(33, 120, 120), GraphShape::new(16, 120, 120)),
                ("decoder", GraphShape::new(16, 120, 120), GraphShape::new(3, 960, 960)),
            ]
        );

        // Resolution-preserving: the region comes back at its own size, restored rather than enlarged — and as the
        // decoder's own planar float, three planes of it, since the depth is the pipeline's to apply once.
        assert_eq!(restored.len(), 3 * 960 * 960);
    }

    #[test]
    fn an_extent_that_cannot_fill_one_region_is_refused_before_any_graph_runs() {
        // The tensor is always exactly one region — the published graphs carry no symbolic dimension on any axis — so
        // what the extent says is how much of it comes from the picture and how much is mirrored. An empty extent has
        // no pixel to mirror, and one larger than a region would silently drop the pixels past its edge. Checked here
        // rather than left to the runtime, which reports the first as an assertion inside the pad and the second not
        // at all.
        let calls: Calls = Arc::default();
        let held = graphs(&calls, None);
        let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };

        let source = source();
        for (width, height) in [(0, 960), (960, 0), (0, 0), (961, 960), (960, 961), (1024, 1024)] {
            let error = restore_region::<Fake>(
                &set,
                &mut RegionScratch::default(),
                &Sampler::new(&source),
                Tile { x: 0, y: 0, width, height },
            )
            .expect_err("an extent that cannot fill a region was restored");

            assert!(matches!(error, RegionError::WrongSize { .. }), "{width}x{height}: {error}");
            assert!(error.to_string().contains("960"), "the error did not name the size it accepts: {error}");
        }

        assert!(calls.lock().unwrap().is_empty(), "a graph ran for an extent that cannot fill a region");
    }

    #[test]
    fn an_extent_short_of_a_region_is_mirrored_out_to_one_and_runs() {
        // Which is what the alignment extension and an image smaller than one region both look like from here: the
        // driver clips the tile to the picture and the conversion mirrors the rest out, so the tensor is 960x960
        // whatever the extent was.
        let source = source();

        for (width, height) in [(1, 1), (17, 960), (960, 17), (952, 944)] {
            let calls: Calls = Arc::default();
            let held = graphs(&calls, None);
            let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };

            let mut scratch = RegionScratch::default();
            let restored =
                restore_region::<Fake>(&set, &mut scratch, &Sampler::new(&source), Tile { x: 0, y: 0, width, height })
                    .unwrap_or_else(|error| panic!("a {width}x{height} extent was refused: {error}"));

            assert_eq!(restored.len(), 3 * 960 * 960, "a {width}x{height} extent did not fill a region");
            assert_eq!(
                calls.lock().unwrap()[0],
                ("encoder", GraphShape::new(3, 960, 960), GraphShape::new(16, 120, 120)),
                "a {width}x{height} extent reached the encoder at some other shape"
            );
        }
    }

    #[test]
    fn a_failure_in_any_one_graph_produces_no_region_at_all() {
        // Not the stages that had already run, and not a half-written buffer: half a restored latent beside the
        // previous region's tail is still an image, and nothing downstream could tell it from a finished one.
        for (role, ran_before) in [("encoder", 1), ("transformer", 2), ("decoder", 3)] {
            let calls: Calls = Arc::default();
            let held = graphs(&calls, Some(role));
            let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };

            let source = source();
            let error = restore_region::<Fake>(&set, &mut RegionScratch::default(), &Sampler::new(&source), region())
                .expect_err("a region survived a failing graph");

            let RegionError::Graph { role: named, .. } = &error else {
                panic!("a failing {role} reported {error} rather than a graph failure");
            };
            assert_eq!(*named, role, "the failure named the wrong graph");

            // It stopped there rather than running on: a later stage fed a buffer the failed one never wrote would
            // be a region assembled from the previous one's numbers.
            assert_eq!(calls.lock().unwrap().len(), ran_before, "the region continued past the failing {role}");
        }
    }

    #[test]
    fn the_same_region_of_the_same_image_restores_identically_twice() {
        // The noise field is keyed on the region's origin, so a run is reproducible — which is what the run cache's
        // correctness rests on, and what a fresh seed per run would take away.
        let calls: Calls = Arc::default();
        let held = graphs(&calls, None);
        let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };
        let source = source();

        let restore = || -> Vec<f32> {
            restore_region::<Fake>(&set, &mut RegionScratch::default(), &Sampler::new(&source), region())
                .expect("the fake restores a region")
                .to_vec()
        };

        assert_eq!(restore(), restore());
    }

    #[test]
    fn the_scratch_is_reusable_across_regions_without_carrying_one_into_the_next() {
        // It is reused rather than reallocated per region, so the property that makes that safe — every buffer fully
        // overwritten before it is read again — is worth checking rather than asserting in a comment.
        let calls: Calls = Arc::default();
        let held = graphs(&calls, None);
        let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };
        let source = source();
        let mut shared = RegionScratch::default();

        let first = restore_region::<Fake>(&set, &mut shared, &Sampler::new(&source), region())
            .expect("first region")
            .to_vec();
        let again = restore_region::<Fake>(&set, &mut shared, &Sampler::new(&source), region())
            .expect("same region")
            .to_vec();
        let fresh = restore_region::<Fake>(&set, &mut RegionScratch::default(), &Sampler::new(&source), region())
            .expect("the same region on untouched scratch")
            .to_vec();

        assert_eq!(again, fresh, "the reused scratch carried something into the next region");
        assert_eq!(first, fresh);
    }

    #[test]
    fn the_region_carries_the_decoders_own_precision_rather_than_a_channel_depth() {
        // Neither depth is applied here, which is what the accumulator this feeds needs: it averages every region
        // that covered a pixel, and quantising a stochastic region on its way into that average throws away precision
        // exactly where the averaging needs it. The depth is applied once, to the finished picture.
        let calls: Calls = Arc::default();
        let held = graphs(&calls, None);
        let set = RegionGraphs::<Fake> { encoder: &held[0], transformer: &held[1], decoder: &held[2] };
        let source = source();

        let mut scratch = RegionScratch::default();
        let restored = restore_region::<Fake>(&set, &mut scratch, &Sampler::new(&source), region())
            .expect("the fake restores a region");

        assert_eq!(restored.len(), 3 * 960 * 960, "the region is not three planes of one region");

        // Values a channel depth could not have held: the fake's output is the mean of what arrived, which lands
        // between the levels either depth quantises to.
        assert!(
            restored.iter().any(|value| (value * 255.0).fract().abs() > 1e-4),
            "every value landed on an 8-bit level, so something quantised on the way out"
        );
    }
}
