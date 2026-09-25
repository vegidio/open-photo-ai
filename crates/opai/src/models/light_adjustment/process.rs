//! The light adjustment contract both of this family's models run: one fixed square, one graph run, and the answer
//! applied to the photograph's own pixels as a per-channel gain.

// Family tier rather than either model's, because both variants adjust through it unchanged. What is left in a
// model's own directory is the square its graph was exported at, the measured profile, and the prose behind both.
//
// What a run is:
//
//   plan        the longest side onto the variant's square, rounded, and the extension that fills the rest
//   present     Lanczos3 to the planned size, then reflection-padded into the graph's square as planar CHW
//   run         one graph run: the square in, the square out
//   correct     the padding dropped, both low-resolution images upsampled, and a per-channel gain applied
//   blend       the photograph moved towards — or away from — the corrected image by the run's bias
//
// **Two of those five are not this family's.** The plan and the presentation are `imaging::present` and the blend
// is `imaging::mix`, because a fixed square and a per-run control are what every whole-image family does rather
// than what this one decided — colour balance reaches its own graphs through both. What is written here is what only
// light adjustment answers: `RANGE`, the range its graphs read; `corrected` and `gain`, the gain map; `STEPS`, its
// progress schedule; and `Adjust`, the pipeline that orders them. A second family fitting a polynomial instead of a
// gain shares every line of the first list and none of the second.
//
// **The model's output is never what comes back.** It is produced at the square whatever the photograph's size is,
// so returning it enlarged would cap every photograph's detail at 1024 pixels on its longest side. What the model
// contributes is the *change* in light, taken as the ratio between what it produced and what it was shown, and
// applied to pixels the photograph already had. That is the whole of why a fixed square is affordable here.

use std::sync::Arc;

use image::imageops::FilterType;
use image::{DynamicImage, ImageBuffer, Rgb};

use super::LightAdjustmentParams;

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::pipeline::{ImagePipeline, OnOneGraph, Shared, SingleGraph, checkpoint, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::ChannelDepth;
use imaging::mix::blended;
use imaging::present::{plan, presented};
use imaging::tensor::{self, Channel, Normalisation, Sampler};

// `[0, 1]` is the reference's `standardize: false` on **both** halves of its conversion — against the `[-1, 1]` face
// recovery's restorers were trained on. Pinned as a literal here and asserted in the tests below, because feeding a
// graph the wrong range is not an error a runtime reports: it is a worse photograph.
/// The range this family's graphs read and write: `[0, 1]`.
pub(super) const RANGE: Normalisation = Normalisation::Unit;

// The reference writes `eps * 65535.0` because Go's colour accessor returns values on a `0..65535` scale and picked
// the scale for it; the scale cancels in the ratio, so the same arithmetic written over `Channel::to_unit`'s `[0, 1]`
// needs the bare constant and is identical at both depths.
/// Guards the gain's denominator so that a near-black pixel cannot produce an unbounded correction.
///
/// `1e-3` in `[0, 1]` unit space.
const EPS: f32 = 1e-3;

/// The photograph's own pixels, corrected by the ratio between what the model produced and what it was shown.
///
/// `shown` is the resampled photograph [`presented`] handed back, `produced` is the model's output with its
/// extension already dropped, and both are at the planned low resolution. Both are upsampled to the photograph's own
/// size and the ratio is taken **per pixel at full resolution**:
///
/// ```text
///   result = source * upsampled(produced) / (upsampled(shown) + EPS)
/// ```
pub(super) fn corrected<T: Channel>(
    source: &Sampler<'_>,
    shown: &DynamicImage,
    produced: &DynamicImage,
    extent: (u32, u32),
) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // `shown` cannot be replaced by the photograph itself. It is the obvious simplification and it undoes the whole
    // arrangement: the ratio would then be exact per pixel and the result would be `upsampled(produced)` — the
    // low-resolution model output enlarged, which is the one thing the gain map exists to avoid. What makes detail
    // finer than the square survive is that the numerator and the denominator are both low-passed, so their ratio
    // carries only the change in light and the photograph supplies everything else.
    //
    // Nor is the ratio taken at the square and upsampled once. That halves the scratch and saves a resample, and it is
    // **not the same arithmetic**: Lanczos is linear, so `Lanczos(out) / Lanczos(in)` is not `Lanczos(out / in)`. The
    // two diverge exactly at edges, where `in` is small and the ratio is steep — which is where ringing in an upsampled
    // ratio plane would show. Every canvas-size measurement in the two model files was taken against the form written
    // here, so adopting the cheaper one is a change that must re-take them.
    let (width, height) = extent;

    // Bound rather than inlined: both are photograph-sized buffers the samplers below borrow for the whole loop. The
    // two resamples read nothing of each other, so the second runs beside the first rather than after it.
    let (shown, produced) = std::thread::scope(|scope| {
        let shown = scope.spawn(|| shown.resize_exact(width, height, FilterType::Lanczos3));
        let produced = produced.resize_exact(width, height, FilterType::Lanczos3);

        (shown.join().expect("a resample does not panic"), produced)
    });

    let (shown, produced) = (Sampler::new(&shown), Sampler::new(&produced));

    ImageBuffer::from_fn(width, height, |x, y| {
        let full = source.rgb(x, y);
        let before = shown.rgb(x, y);
        let after = produced.rgb(x, y);

        // `from_unit` bounds each channel to the range it can carry, which is the other half of the near-black
        // guard: `EPS` keeps the gain finite and this keeps the product inside the channel.
        Rgb([0, 1, 2].map(|channel| {
            T::from_unit(gain(full[channel].to_unit(), before[channel].to_unit(), after[channel].to_unit()))
        }))
    })
}

/// One channel through the gain: `full * produced / (shown + EPS)`, in `[0, 1]` unit space.
fn gain(full: f32, shown: f32, produced: f32) -> f32 {
    full * (produced / (shown + EPS))
}

// Counted rather than accumulated, so the last report lands on exactly the end of the operation's share instead of on
// whatever a run of additions of a quarter drifted to. That is `face_recovery::restore`'s own schedule and for the
// same reason.
//
// Four even steps rather than the reference's `0 / 0.9 / 1`, because those three constants describe no run. The graph
// is a **fixed** square cost whatever the photograph is, while everything after it — two full-resolution resamples
// and two full-resolution loops — is proportional to the photograph. On a 0.4-megapixel source the graph dominates
// and on a 24-megapixel one it is noise, so no constant split is right at both ends, and a bar that advanced only
// around the graph would sit at one figure for most of the time a large photograph takes.
//
// A weighting fitted to the measured split is rejected: it would be a constant correct for one image size and one
// provider, derived from numbers nobody could re-derive from the code.
/// How many progress steps one run is: presented, run, corrected, blended.
const STEPS: usize = 4;

/// The pipeline running one light adjustment operation, as the family's contract seam hands it back.
pub(crate) fn pipeline<B: Backend>(
    name: String,
    artifact: ArtifactId,
    profile: EpProfile,
    params: LightAdjustmentParams,
    canvas: u32,
) -> Shared<B> {
    Arc::new(Adjust::new(name, artifact, profile, params, canvas))
}

/// One light adjustment run: the graph it opens, the settings it opens it under, the square it runs at, and how far
/// the photograph is moved towards what came back.
struct Adjust {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
    /// The square this variant's graph was exported at.
    canvas: u32,
    /// How far, and in which direction, the photograph is moved towards the corrected image.
    bias: f32,
}

impl Adjust {
    fn new(name: String, artifact: ArtifactId, profile: EpProfile, params: LightAdjustmentParams, canvas: u32) -> Self {
        Self { graph: SingleGraph::new(name, artifact, profile), canvas, bias: params.bias.as_f32() }
    }
}

impl OnOneGraph for Adjust {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> ImagePipeline<B> for Adjust {
    /// One graph run, whatever the photograph is.
    fn stages(&self) -> usize {
        // The fixed square is the whole of why this is a constant rather than something derived from the image: a
        // thumbnail and a 24-megapixel photograph are both one run.
        1
    }

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError> {
        // Dispatched once at the top, as `restore` dispatches it, so the two full-resolution loops below are
        // compiled per channel type rather than branching per pixel over twenty-four million of them.
        match depth {
            ChannelDepth::Eight => self.adjusted::<u8, B>(input, sessions, progress, cancelled).map(u8::into_dynamic),
            ChannelDepth::Sixteen => {
                self.adjusted::<u16, B>(input, sessions, progress, cancelled).map(u16::into_dynamic)
            }
        }
    }
}

impl Adjust {
    /// [`ImagePipeline::run`]'s body, once, at whichever channel the caller asked for.
    fn adjusted<T: Channel, B: Backend>(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImageBuffer<Rgb<T>, Vec<T>>, InferenceError>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let (width, height) = (input.width(), input.height());

        // Before anything is allocated. There is no scaling of an empty photograph onto the square, and a graph run
        // over a square holding nothing but a reflection of nothing is not an adjustment of anything.
        if width == 0 || height == 0 {
            return Err(InferenceError::Untileable { width, height });
        }

        let report = reporter(progress);
        let planned = plan(width, height, self.canvas);
        let shape = GraphShape::new(3, self.canvas as usize, self.canvas as usize);

        // Checked at each of the four step boundaries, which is the only schedule a four-step run offers. A
        // cancelled run returns no image at all — not the photograph, and not the partly corrected buffer the
        // cancellation landed in, which a caller has no way to tell from a finished adjustment.
        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        let mut tensor = vec![0.0_f32; shape.len()];
        // `expect` rather than a folded error, as `run_tiled` does with its own conversion and for the same reason:
        // the scratch is allocated here at exactly the shape the graph is run at, so a disagreement is this
        // function contradicting itself rather than anything a caller could have caused or acted on.
        let shown = presented(input, planned, RANGE, &mut tensor)
            .expect("the scratch is allocated at the square the graph accepts");

        checkpoint(&report, cancelled, 1.0 / STEPS as f64)?;

        let mut output = vec![0.0_f32; shape.len()];
        B::run_graph(&sessions[0], &tensor, shape, &mut output, shape)
            .map_err(InferenceError::run(&self.graph.name, 0))?;

        checkpoint(&report, cancelled, 2.0 / STEPS as f64)?;

        // The extension dropped **here**, before the upsample: cropping afterwards would mean resampling the mirror
        // of the photograph's own edge into the pixels that are kept, at an aspect ratio that is the canvas's.
        let mut low = ImageBuffer::<Rgb<T>, Vec<T>>::new(planned.scaled_width, planned.scaled_height);
        tensor::padded_chw_to_image(&mut low, &output, (self.canvas, self.canvas), RANGE)
            .expect("the planned size is inside the square the graph returned");

        let produced = T::into_dynamic(low);
        let sampler = Sampler::new(input);
        let corrected = corrected::<T>(&sampler, &shown, &produced, (width, height));

        checkpoint(&report, cancelled, 3.0 / STEPS as f64)?;

        let blended = blended(&sampler, corrected, self.bias);

        report(1.0);

        Ok(blended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use imaging::test_support::photograph;

    #[test]
    fn the_graph_is_fed_the_unit_range_rather_than_the_signed_one_face_recovery_uses() {
        // The literal, pinned rather than left to be read off a call, for the reason `RANGE` gives.
        assert_eq!(RANGE, Normalisation::Unit, "the family's graphs were switched to the signed range");

        // And the property behind the literal: a presented photograph lands inside `[0, 1]`, where `Signed` would
        // have put half of it below zero.
        let canvas = 8;
        let mut tensor = vec![0.0_f32; 3 * (canvas as usize) * (canvas as usize)];
        presented(&photograph(20, 11), plan(20, 11, canvas), RANGE, &mut tensor).expect("a square scratch");

        assert!(
            tensor.iter().all(|value| (0.0..=1.0).contains(value)),
            "a presented photograph left the range the graph was trained on"
        );
    }

    /// The correction of `source` where the model returned `shown` scaled by `factor` in every channel.
    ///
    /// Built by scaling the low-resolution image the model was shown, which is what makes the expected answer
    /// readable: the gain is then that factor everywhere, whatever the photograph contains.
    fn corrected_by<T: Channel>(source: &DynamicImage, shown: &DynamicImage, factor: f32) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let sampler = Sampler::new(shown);
        let produced = DynamicImage::ImageRgb16(ImageBuffer::from_fn(shown.width(), shown.height(), |x, y| {
            let [r, g, b] = sampler.rgb(x, y);

            Rgb([r, g, b].map(|value| u16::from_unit(value.to_unit() * factor)))
        }));

        corrected(&Sampler::new(source), shown, &produced, (source.width(), source.height()))
    }

    #[test]
    fn a_model_output_equal_to_its_input_leaves_the_photograph_alone() {
        // The identity the whole arrangement rests on: when the model changes nothing, the gain is one everywhere
        // and what comes back is the photograph's own pixels. A correction that had upsampled the model's output
        // instead of dividing by what it was shown would return a blurred picture here and pass no test but this.
        // A smooth photograph well clear of black, because the identity is exact only where the denominator's guard
        // is negligible against the pixel — which is the whole of what `EPS` is for, and is checked on its own
        // below. The modular fixture above low-passes to near-zero in places, where the guard legitimately bites.
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_fn(37, 23, |x, y| {
            Rgb([(80 + x * 3) as u8, (90 + y * 4) as u8, (100 + x + y) as u8])
        }));
        let shown = source.resize_exact(16, 10, FilterType::Lanczos3);

        let produced: ImageBuffer<Rgb<u8>, Vec<u8>> = corrected_by(&source, &shown, 1.0);
        let original = Sampler::new(&source);

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = original.rgb(x, y);
            let expected = [r, g, b].map(|value| u8::from_unit(value.to_unit()));

            // Within two levels rather than bit-for-bit: the identity passes through the `+ EPS` guard, which is
            // 0.3% at these values, and through a division of a 16-bit upsample by an 8-bit one. What is being
            // asserted is that the photograph comes back, not that the arithmetic is lossless.
            for (channel, want) in expected.into_iter().enumerate() {
                let (got, want) = (i32::from(pixel.0[channel]), i32::from(want));

                assert!((got - want).abs() <= 2, "({x}, {y}) channel {channel}: {got} against the photograph's {want}");
            }
        }
    }

    #[test]
    fn a_uniform_brightening_scales_every_channel_by_that_factor() {
        // What the gain map *is*, stated on the simplest model output that is not the identity. The factor is read
        // off the result rather than assumed, so a correction that applied the ratio in the wrong direction — a
        // plausible mistake, and a photograph rather than an error — fails here.
        let source = photograph(29, 19);
        let shown = source.resize_exact(12, 8, FilterType::Lanczos3);

        for factor in [0.5_f32, 1.25, 1.75] {
            let produced: ImageBuffer<Rgb<u16>, Vec<u16>> = corrected_by(&source, &shown, factor);
            let original = Sampler::new(&source);

            for (x, y, pixel) in produced.enumerate_pixels() {
                let [r, _, _] = original.rgb(x, y);
                let expected = (r.to_unit() * factor).clamp(0.0, 1.0);

                assert!(
                    (pixel.0[0].to_unit() - expected).abs() < 0.02,
                    "at {factor}x, ({x}, {y}) landed on {} rather than {expected}",
                    pixel.0[0].to_unit()
                );
            }
        }
    }

    #[test]
    fn a_near_black_region_stays_in_range_rather_than_being_amplified_without_bound() {
        // The denominator's guard, on the input that reaches it: a channel at zero divides by `EPS` alone, so a
        // model output of any size over it is a finite number — and `from_unit` bounds what is left.
        let black = DynamicImage::ImageRgb16(ImageBuffer::from_pixel(20, 20, Rgb([0_u16, 0, 3])));
        let shown = black.resize_exact(8, 8, FilterType::Lanczos3);
        let bright = DynamicImage::ImageRgb16(ImageBuffer::from_pixel(8, 8, Rgb([u16::MAX; 3])));

        let produced: ImageBuffer<Rgb<u16>, Vec<u16>> = corrected(&Sampler::new(&black), &shown, &bright, (20, 20));

        for pixel in produced.pixels() {
            for channel in pixel.0 {
                assert!(channel.to_unit().is_finite(), "a near-black pixel produced a value outside the channel");
            }

            // And the photograph's own near-black stays near black: the gain multiplies what was there, so a pixel
            // with nothing in it has nothing to amplify. This is the property that keeps a shadow from turning into
            // a flat grey patch.
            assert_eq!(pixel.0[0], 0, "a black channel was lifted by a correction rather than multiplied");
            assert!(pixel.0[2] > 0, "a near-black channel was not corrected at all");
        }
    }

    #[test]
    fn detail_finer_than_the_square_survives_the_correction() {
        // The requirement the fixed square is affordable under, and the one that separates this arrangement from
        // returning the model's output enlarged. A one-pixel checkerboard is entirely lost by the downscale to the
        // square, so a result carrying it can only have come from the photograph's own pixels.
        let checker = DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |x, y| {
            Rgb(if (x + y) % 2 == 0 { [230_u8; 3] } else { [40; 3] })
        }));
        let shown = checker.resize_exact(8, 8, FilterType::Lanczos3);

        let produced: ImageBuffer<Rgb<u8>, Vec<u8>> = corrected_by(&checker, &shown, 1.1);

        // The alternation is still there, pixel by pixel, rather than averaged into the flat grey an 8x8 model
        // output enlarged back to 64x64 would be.
        let mut spread = 0_i32;
        for y in 0..64 {
            for x in 0..63 {
                let here = i32::from(produced.get_pixel(x, y).0[0]);
                let next = i32::from(produced.get_pixel(x + 1, y).0[0]);

                spread = spread.max((here - next).abs());
            }
        }

        assert!(spread > 100, "the checkerboard was flattened to a spread of {spread} levels");
    }

    #[test]
    fn a_sixteen_bit_correction_is_not_computed_through_eight_bit_intermediates() {
        // The crate's rule, and the family where following the reference would have broken it: it forces every
        // intermediate to 8-bit RGBA, so a 16-bit source computes its gain map from 256 levels and then writes that
        // answer into a 16-bit result. A gradient finer than an 8-bit channel can hold is what shows the difference.
        let gradient = DynamicImage::ImageRgb16(ImageBuffer::from_fn(600, 1, |x, _| Rgb([(x as u16) * 100; 3])));
        let shown = gradient.resize_exact(64, 1, FilterType::Lanczos3);

        let produced: ImageBuffer<Rgb<u16>, Vec<u16>> = corrected_by(&gradient, &shown, 1.0);
        let levels: std::collections::BTreeSet<u16> = produced.pixels().map(|pixel| pixel.0[0]).collect();

        assert!(levels.len() > 256, "a 16-bit correction carried only {} distinct levels", levels.len());
    }

    // The fake-backend suite: the whole pipeline — the plan, the presentation, the crop, the correction, the blend,
    // the progress schedule, the depth dispatch, the cancellation and the refusal — with no ONNX Runtime, no GPU, no
    // model file and no network, which is what keeps every property above checked on every CI platform.

    use std::sync::Mutex;

    use super::super::{LightAdjustment, LightAdjustmentVariant};

    use crate::error::SessionError;
    use crate::models::Bias;
    use crate::models::precision::FloatPrecision;
    use crate::pipeline::test_support::stub_backend_runs;
    use crate::providers::ExecutionProvider;

    // The square is the variant's rather than the contract's, which is exactly what makes this substitutable: a test
    // that had to run at 1024 would be a megabyte of scratch and a second of arithmetic per case.
    /// A canvas small enough for a test to run a whole pipeline over, in place of the 1024 both variants ship at.
    const SMALL: u32 = 16;

    /// What the fake graph was asked to do, so a test can say what reached it rather than what came back.
    #[derive(Default)]
    struct Log {
        /// The two shapes each run declared, so a run at the wrong square is a failure rather than a silent resize.
        shapes: Vec<(GraphShape, GraphShape)>,
        /// How many times the graph was run, which for this contract must be once.
        runs: usize,
    }

    /// A session standing in for an adjustment graph: it records what it was given and answers with the input
    /// scaled by a known factor.
    struct FakeSession {
        // Scaled rather than filled with a constant, because a constant output makes the gain map's denominator
        // irrelevant — the correction would then be the same whatever it divided by, and the one step this suite is
        // least able to check elsewhere would go unexercised. Nothing here is presented as an adjustment.
        log: Arc<Mutex<Log>>,
        /// What every element of the output is multiplied by, in the graph's own `[0, 1]` range.
        scales: f32,
    }

    /// A backend with **no ONNX Runtime**.
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
            unreachable!("the pipeline is handed its sessions; it never acquires one")
        }

        stub_backend_runs!("an adjustment run takes a declared-shape seam"; run_tile);

        /// The single-input call, which is the only one either of this family's graphs takes.
        fn run_graph(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            output: &mut [f32],
            output_shape: GraphShape,
        ) -> Result<(), Self::Error> {
            let session = handle.session();

            {
                let mut log = session.log.lock().unwrap();
                log.shapes.push((input_shape, output_shape));
                log.runs += 1;
            }

            // Refused exactly as the real seam does: a buffer the declared shape does not describe is the mistake
            // it exists to prevent.
            assert_eq!(input.len(), input_shape.len(), "the graph was fed a buffer {input_shape} does not describe");
            assert_eq!(output.len(), output_shape.len(), "the output buffer is not {output_shape}");

            for (value, source) in output.iter_mut().zip(input) {
                *value = (source * session.scales).clamp(0.0, 1.0);
            }

            Ok(())
        }

        stub_backend_runs!("an adjustment run takes one output"; run_named_outputs);
        stub_backend_runs!("neither of this family's graphs takes a second input"; run_weighted);
    }

    /// A handle on a fake session scaling by `scales`, and the log it records into.
    fn session(scales: f32) -> (SessionHandle<FakeSession>, Arc<Mutex<Log>>) {
        let log: Arc<Mutex<Log>> = Arc::default();
        let handle = SessionHandle::held(FakeSession { log: Arc::clone(&log), scales }, ExecutionProvider::Cpu);

        (handle, log)
    }

    /// The pipeline at [`SMALL`], at `bias`.
    fn adjusting(bias: f64) -> Shared<Fake> {
        // The name and the artifact come off a real operation rather than being written here, so what these tests
        // hold is what the family's seam will hand over — the square alone is substituted, and it is the one thing a
        // variant is entitled to differ in.
        let bias = Bias::new(bias).expect("the test supplied a bias in range");
        let operation = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias);

        pipeline::<Fake>(operation.display_name(), operation.artifact(), operation.profile(), operation.params(), SMALL)
    }

    #[test]
    fn one_run_is_one_graph_run_at_the_variants_own_square() {
        // The contract's whole shape, and the two halves a caller cannot see: the graph is run once whatever the
        // photograph is, and it is run at the square the variant declares rather than at anything derived from the
        // image. A pipeline that had passed the photograph's own dimensions would still produce a picture here.
        let (handle, log) = session(1.0);

        let produced = adjusting(1.0)
            .run(&photograph(53, 31), std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("an adjustment over a photograph");

        assert_eq!((produced.width(), produced.height()), (53, 31), "the result is not the photograph's own size");

        let log = log.lock().unwrap();
        assert_eq!(log.runs, 1, "a fixed-canvas contract ran the graph {} times", log.runs);
        assert_eq!(
            log.shapes[0],
            (
                GraphShape::new(3, SMALL as usize, SMALL as usize),
                GraphShape::new(3, SMALL as usize, SMALL as usize)
            ),
            "the graph was not run at the variant's square"
        );

        // And `stages` agrees with what actually happened, which is what the driver's per-step record reports.
        assert_eq!(ImagePipeline::<Fake>::stages(adjusting(1.0).as_ref()), log.runs);
    }

    #[test]
    fn progress_reaches_exactly_the_end_of_the_range_and_never_goes_backwards() {
        // Counted rather than accumulated — see `STEPS` — and the property `light-adjustment`'s spec states, that the
        // report advances during the full-resolution work rather than only around the graph.
        let (handle, _) = session(1.2);
        let reported: Mutex<Vec<f64>> = Mutex::default();

        adjusting(0.5)
            .run(
                &photograph(40, 60),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                Some(&|fraction| reported.lock().unwrap().push(fraction)),
                &|| false,
            )
            .expect("an adjustment over a photograph");

        let reported = reported.lock().unwrap();

        assert_eq!(reported.len(), STEPS, "a four-step run reported {} times", reported.len());
        assert!(reported.windows(2).all(|pair| pair[1] > pair[0]), "progress went backwards: {reported:?}");
        assert!(reported[0] > 0.0, "the first report was at the start of the range rather than after a step");
        assert_eq!(*reported.last().expect("a run reports"), 1.0, "the last report was not the end of the range");

        // More than one report lands **after** the graph has run, which is the half of the schedule the reference's
        // `0 / 0.9 / 1` does not have: on a large photograph everything after the graph is most of the run.
        assert!(reported.iter().filter(|fraction| **fraction > 0.5).count() >= 2, "{reported:?}");
    }

    #[test]
    fn a_cancellation_at_any_of_the_four_boundaries_produces_no_image() {
        // Every boundary, separately, rather than one of them: a caller handed a partly corrected photograph with
        // no error has no way to tell it from a finished adjustment, and the three later boundaries are the ones
        // where such a buffer exists to be handed back.
        for boundary in 0..STEPS {
            let (handle, _) = session(1.0);
            let checked = std::sync::atomic::AtomicUsize::new(0);

            let outcome = adjusting(1.0).run(
                &photograph(33, 21),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                None,
                &|| checked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= boundary,
            );

            assert!(
                matches!(outcome, Err(InferenceError::Cancelled)),
                "a cancellation at boundary {boundary} produced {outcome:?}"
            );
        }

        // And the run that is never cancelled still returns a photograph, so the loop above is checking a refusal
        // rather than a pipeline that fails whatever it is told.
        let (handle, _) = session(1.0);
        assert!(
            adjusting(1.0)
                .run(&photograph(33, 21), std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .is_ok()
        );
    }

    #[test]
    fn an_image_with_no_area_is_refused_before_anything_is_allocated() {
        // The same refusal the tiling of a zero-area image makes, for the same reason. Checked on both axes: a
        // guard on one of them would let the other reach the plan's assertion as a panic.
        for (width, height) in [(0, 8), (8, 0), (0, 0)] {
            let (handle, log) = session(1.0);
            let empty = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let outcome =
                adjusting(1.0).run(&empty, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false);

            assert!(
                matches!(outcome, Err(InferenceError::Untileable { .. })),
                "a {width}x{height} image produced {outcome:?}"
            );
            assert_eq!(log.lock().unwrap().runs, 0, "a refused run reached the graph");
        }
    }

    #[test]
    fn a_run_produces_the_channel_depth_that_was_asked_for() {
        // The depth dispatch, which is the divergence from the reference this pipeline is built around: it forces
        // every intermediate to 8-bit RGBA, so a 16-bit photograph would compute its gain map from 256 levels.
        let source = DynamicImage::ImageRgb16(ImageBuffer::from_fn(40, 25, |x, y| {
            Rgb([(x * 700 + y * 11) as u16, (x * 13 + y * 900) as u16, 30_000])
        }));

        for (depth, sixteen) in [(ChannelDepth::Eight, false), (ChannelDepth::Sixteen, true)] {
            let (handle, _) = session(1.1);
            let produced = adjusting(1.0)
                .run(&source, std::slice::from_ref(&handle), depth, None, &|| false)
                .expect("an adjustment over a photograph");

            assert_eq!((produced.width(), produced.height()), (40, 25));
            assert_eq!(produced.as_rgb16().is_some(), sixteen, "{depth:?} did not produce the depth asked for");
            assert_eq!(produced.as_rgb8().is_some(), !sixteen, "{depth:?} did not produce the depth asked for");

            // And colour only: alpha is not preserved through a model run, and a fourth channel would claim the
            // model returned an opacity it was never given.
            assert!(produced.as_rgba8().is_none() && produced.as_rgba16().is_none(), "{depth:?} carried an alpha");
        }
    }

    #[test]
    fn a_photograph_carrying_transparency_comes_back_as_colour_alone() {
        // `light-adjustment`'s spec scenario, on the one input variant that has an alpha to lose.
        let (handle, _) = session(1.0);
        let translucent = DynamicImage::ImageRgba8(ImageBuffer::from_fn(20, 20, |x, y| {
            image::Rgba([(x * 9) as u8, (y * 9) as u8, 120, 128])
        }));

        let produced = adjusting(1.0)
            .run(&translucent, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("an adjustment over a translucent photograph");

        assert!(produced.as_rgb8().is_some(), "a translucent photograph did not come back as colour alone");
    }

    #[test]
    fn a_bias_of_zero_returns_the_photograph_through_the_whole_pipeline() {
        // The end-to-end statement of what `imaging::mix`'s endpoint test checks on the blend alone, and the one that
        // would catch a bias read from the wrong place or applied before the gain rather than after it: a graph that
        // brightens by a fifth changes nothing at all when the run's bias is zero.
        let (handle, _) = session(1.2);
        let source = photograph(30, 22);

        let produced = adjusting(0.0)
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("an adjustment at a bias of zero");

        let original = Sampler::new(&source);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = original.rgb(x, y);

            assert_eq!(
                pixel.0,
                [r, g, b].map(|value| u8::from_unit(value.to_unit())),
                "a bias of zero changed ({x}, {y})"
            );
        }
    }

    #[test]
    fn two_biases_of_opposite_sign_move_the_photograph_in_opposite_directions() {
        // `light-adjustment`'s spec scenario, through the whole pipeline rather than the blend alone. Mean luminance,
        // because what the bias does is a direction rather than a per-pixel value, and the graph here brightens.
        let (brighter, _) = session(1.3);
        let (darker, _) = session(1.3);
        let source = photograph(30, 22);

        let mean = |image: &DynamicImage| {
            let sampler = Sampler::new(image);
            let total: f64 = (0..22)
                .flat_map(|y| (0..30).map(move |x| (x, y)))
                .map(|(x, y)| f64::from(sampler.rgb(x, y)[0]))
                .sum();

            total / (30.0 * 22.0)
        };

        let up = adjusting(1.0)
            .run(&source, std::slice::from_ref(&brighter), ChannelDepth::Eight, None, &|| false)
            .expect("a positive bias");
        let down = adjusting(-1.0)
            .run(&source, std::slice::from_ref(&darker), ChannelDepth::Eight, None, &|| false)
            .expect("a negative bias");

        let (before, up, down) = (mean(&source), mean(&up), mean(&down));

        assert!(up > before, "a positive bias did not brighten: {up} against {before}");
        assert!(down < before, "a negative bias did not darken: {down} against {before}");
    }
}
