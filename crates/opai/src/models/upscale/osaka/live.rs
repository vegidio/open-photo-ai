//! The live checks: the real Osaka weights, the real ONNX Runtime, one real region.
//!
//! Everything else in this module is arithmetic exercised on every CI platform against a fake backend — the packing,
//! the scheduler step, the noise field, the shapes each graph is called with, the refusals. What cannot be is the
//! three things this slice exists to answer, none of which is reachable without the real weights and two of which
//! fail *silently* if they are wrong.
//!
//! # Why they are `#[ignore]`d
//!
//! They need **~7.2 GB** of Osaka FP16 weights, which CI has neither the storage nor the minutes for, and a region on
//! the CPU provider is slow enough that nothing about it belongs in an ordinary sweep. The same arrangement, for the
//! same reason, as the chain's own live checks and [`crate::sessions`]'s. Run them by hand:
//!
//! ```text
//! cargo test -p opai models::upscale::osaka::live -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The whole module is about ten minutes on an M2 Max, almost all of it the five CPU regions.
//!
//! They install into a directory they **keep** — [`kept_dir`] — rather than a temporary one, because 7.2 GB
//! re-transferred per test is not a suite anybody runs twice. `OPAI_LIVE_DIR` moves it off the system temporary
//! directory for a runner where several gigabytes belong on another volume.
//!
//! # The region is 960x960 because the graph accepts nothing else
//!
//! The committed fixture is 640x640, so the region checks resample it up to one region first — the same thing the
//! pipeline does for an image smaller than a region, done by hand so that what is under test is the region rather
//! than the driver around it. SeedVR2 is *meant* to be driven this way: the network preserves resolution throughout,
//! so the image is resized to its target and detail is restored at that size.
//!
//! # What running them answered
//!
//! Recorded here rather than left in a terminal, because the figures are the deliverable of this slice.
//!
//! **Every constant design.md D7 took on trust is what the exports declare.** Confirmed by task 1.1 against the
//! published FP16 graphs, with no symbolic dimension on any axis of any of them:
//!
//! ```text
//!   encoder      pixel_image      [1, 3,960,960] f32  ->  latent          [1,16,120,120] f32
//!   transformer  vid_input        [1,33,120,120] f32  ->  denoised_latent [1,16,120,120] f32
//!   decoder      latent           [1,16,120,120] f32  ->  pixel_image     [1, 3,960,960] f32
//! ```
//!
//! **The transformer's load defect is real, and it is exactly what the reference describes.** It fails session
//! creation on the **CPU provider** with every optimizer on, naming a node the rewrite created, and it loads with
//! `SimplifiedLayerNormFusion` disabled. On CoreML it loads either way.
//!
//! This module first recorded something else: that naming `ReshapeFusion` beside it **failed**, and therefore that
//! the pinned runtime takes the setting as one transformer name rather than as a list. It does not. That sweep joined
//! the names with a comma while the runtime separates them with a semicolon, so its multi-name cases were each one
//! unrecognised name — which the runtime ignores in silence, leaving the graph failing exactly as it does with
//! nothing set. The separator is measured directly by
//! [`the_disabled_optimizer_setting_is_a_semicolon_separated_list`], which carries the table.
//!
//! **One region restores, on both providers this machine has, and they agree.** Against `fixtures/test.dat`
//! resampled to 960x960, in a `dev` build on an M2 Max — so the timings compare the two providers and are useless as
//! absolute figures:
//!
//! ```text
//!   one 960x960 region      CoreML     4.38 s
//!                           CPU       75.2  s      17.2x
//!   CPU against CoreML      cosine     0.999998
//! ```
//!
//! The reference records this graph matching the CPU at cosine 0.99999 or better on CoreML; it does. **This is the
//! first diffusion inference this project has ever run**, and the first time a model here has been driven as more
//! than one graph.
//!
//! **Both readings that could have been silently wrong are right, and neither is a close call.** Pearson correlation
//! of the decoded region against the region the encoder was given, on the CPU provider:
//!
//! ```text
//!   as written                   0.981716
//!   scheduler step skipped      -0.865825
//!   channel groups swapped       0.351622
//! ```
//!
//! Skipping the scheduler step scores **negative** — the flow-matching velocity is close to the inverse of the image
//! it was derived from, so decoding it returns something anti-correlated with the input while still being an image,
//! which is exactly the failure that cannot be caught by looking. Swapping the noise and condition groups keeps a
//! third of the structure: enough to look like a bad restoration rather than a bug.
//!
//! **What one region costs in memory: 29.0 GB resident**, against the ~7 GB the three graphs weigh. The rest is
//! activations, with the memory planner off as this model's profile asks. Nothing in this project budgets or refuses
//! that — see [`super`] for the parity gap that records it.
//!
//! **The whole pipeline runs through the public API, and every property it was built for holds.** Over the fixture
//! resampled to 700x520 and upscaled 2x on the CPU provider — four 960x960 regions, 317 s, in a `release` build on an
//! M2 Max:
//!
//! ```text
//!   dimensions        700x520 -> 1400x1040
//!   correlation to the resampled base      0.992214
//!   detail energy     result 0.026307      base 0.013353      1.97x
//!   largest step      across a seam 0.280118      elsewhere 0.708867
//!   channel means     result 0.5632 0.4513 0.3916
//!                     base   0.5633 0.4514 0.3916
//! ```
//!
//! Each line is one of the four things only the assembled image can show. The result carries **twice** the
//! high-frequency detail of the Lanczos base it was conditioned on while still correlating with it at 0.992 — it
//! restored rather than resampled. The largest step across a region boundary is well *below* the largest step the
//! picture has elsewhere, so the weighted accumulator leaves no seam. And the per-channel means track the reference's
//! to four decimal places, which is the colour fix doing the one thing it exists for.
//!
//! **Against the Go application, on the same machine.** Both `perf` apps at FP16 over CoreML, `--runs 5 --scale 4`,
//! over the same 640x640 sample — the two projects' fixtures are byte-identical:
//!
//! ```text
//!                     Rust        Go
//!   output            2560x2560   2560x2560
//!   min               35.701 s    35.380 s
//!   median            44.171 s    38.977 s
//!   max              121.382 s   103.250 s
//! ```
//!
//! The Rust sweep reports five timed inferences rather than a refusal, which is the end-to-end proof that the
//! diffusion refusal is lifted through the public API. The steady-state figures agree to within 1%. Both sweeps ramp
//! to roughly three times their own minimum with resident memory flat, in the same shape — that is the machine
//! throttling under sustained CoreML load rather than either pipeline, which is why the minima are the figures to
//! read. **Pixel equality is not asserted and must not be**: the noise fields differ by design.
//!
//! **The NVIDIA providers remain unverified**, as they are everywhere else in this project. CUDA and TensorRT have
//! never run inference here on any machine, so the per-graph NHWC override's *effect* is unmeasured even though its
//! plumbing is tested.

use image::{DynamicImage, ImageBuffer, Rgb, imageops::FilterType};

use super::latent::{
    LATENT_CHANNELS, REGION_EDGE, REGION_EDGE_PX, REGION_OVERLAP, TRANSFORMER_CHANNELS, VAE_STRIDE, pack,
    padded_extent, scheduler_step,
};
use super::noise::{NOISE_SEED, gaussian};
use super::region::{RegionGraphs, RegionScratch, restore_region};
use crate::ProcessOptions;
use crate::live_support::{self, kept_dir, live_application_at};
use crate::models::{GraphRole, Operation, OsakaPrecision, Resolution, Scale, Upscale, UpscaleVariant};
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::providers::ExecutionProvider;
use crate::providers::profile::{DISABLED_OPTIMIZER_SEPARATOR, EpProfile};
use crate::sessions::SessionHandle;
use crate::{Opai, Picture};
use imaging::grid::{Tile, TileGrid};
use imaging::tensor::{Normalisation, Sampler, chw_to_image, image_to_chw};

/// The application name these install under, and the directory they keep.
const NAME: &str = "opai-live-osaka";

/// The latent edge one region compresses to.
const LATENT_EDGE: usize = REGION_EDGE / VAE_STRIDE;

/// The shapes the three graphs run at, restated here rather than imported so that the differential checks below are
/// driving the exports' own geometry rather than whatever `region.rs` believes today.
const PIXELS: GraphShape = GraphShape::new(3, REGION_EDGE, REGION_EDGE);
const LATENT: GraphShape = GraphShape::new(LATENT_CHANNELS, LATENT_EDGE, LATENT_EDGE);
const PACKED: GraphShape = GraphShape::new(TRANSFORMER_CHANNELS, LATENT_EDGE, LATENT_EDGE);

/// A started application installed into the kept directory.
async fn application() -> Opai {
    live_application_at(NAME, &kept_dir(NAME)).await
}

/// The photograph, resampled to exactly one region.
///
/// Lanczos, which is what the pipeline resamples with, so the pixels this is measured against are the ones the
/// pipeline itself would hand the model.
async fn one_region() -> DynamicImage {
    let picture = live_support::photograph().await;

    let (width, height) = picture.dimensions();
    println!("  source {width}x{height} -> {REGION_EDGE_PX}x{REGION_EDGE_PX} (Lanczos)");

    picture.pixels().resize_exact(REGION_EDGE_PX, REGION_EDGE_PX, FilterType::Lanczos3)
}

/// The whole 960x960 region of `source`.
fn region() -> Tile {
    Tile { x: 0, y: 0, width: REGION_EDGE_PX, height: REGION_EDGE_PX }
}

/// The three sessions for one Osaka FP16 region on `provider`, each under the settings measured for **its** graph.
///
/// Through `resolve()` rather than by naming three artifacts, which is what makes this exercise the per-graph profile
/// contract rather than merely the weights.
async fn sessions(opai: &Opai, provider: ExecutionProvider) -> [SessionHandle<ort::session::Session>; 3] {
    let operation =
        Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Fp16), Scale::new(1.0).expect("1.0 is in range"));
    let Resolution::Graphs(set) = operation.resolve() else { panic!("Osaka resolved to passes") };

    let mut handles = Vec::with_capacity(3);
    for role in [GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder] {
        let graph = set.resolved(role).expect("Osaka declares this role");
        let handle = opai
            .acquire(&graph.artifact, &graph.profile, provider, &crate::sessions::Interest::default())
            .await
            .unwrap_or_else(|err| panic!("{role} did not open on {provider}: {err}"));

        println!("  {role:>12} {} on {:?}", graph.artifact, handle.provider());
        handles.push(handle);
    }

    handles.try_into().unwrap_or_else(|_| unreachable!("three roles produced three handles"))
}

/// The cosine similarity of two equal-length buffers: 1.0 when they point the same way, whatever their magnitudes.
///
/// The measure the reference records its cross-provider agreement in, so the figures here are comparable with its.
fn cosine(left: &[f32], right: &[f32]) -> f64 {
    assert_eq!(left.len(), right.len());

    let (mut dot, mut left_sq, mut right_sq) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (a, b) in left.iter().zip(right) {
        dot += f64::from(*a) * f64::from(*b);
        left_sq += f64::from(*a) * f64::from(*a);
        right_sq += f64::from(*b) * f64::from(*b);
    }

    dot / (left_sq.sqrt() * right_sq.sqrt())
}

/// The Pearson correlation of two equal-length buffers.
///
/// Correlation rather than cosine for the differential checks below, because what they compare is a *restored* region
/// against its input: the restoration legitimately shifts the mean and the scale, and cosine would score that as
/// disagreement. What must survive is the structure, which is what this measures.
fn correlation(left: &[f32], right: &[f32]) -> f64 {
    assert_eq!(left.len(), right.len());
    let count = left.len() as f64;

    let mean_left = left.iter().map(|value| f64::from(*value)).sum::<f64>() / count;
    let mean_right = right.iter().map(|value| f64::from(*value)).sum::<f64>() / count;

    let (mut covariance, mut var_left, mut var_right) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (a, b) in left.iter().zip(right) {
        let (a, b) = (f64::from(*a) - mean_left, f64::from(*b) - mean_right);
        covariance += a * b;
        var_left += a * a;
        var_right += b * b;
    }

    covariance / (var_left.sqrt() * var_right.sqrt())
}

/// Which reading of the region to drive, so that the two that can be silently wrong can be compared against the two
/// that would be wrong instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// What `region.rs` does: `[noise | condition | ones]`, then `x0 = noise - prediction`.
    Correct,
    /// The transformer's output handed to the decoder as though it were the latent — which it is not, whatever the
    /// output tensor is named.
    SkipSchedulerStep,
    /// `[condition | noise | ones]`: the two latent groups exchanged. Nothing in the graph's structure says which is
    /// which, so this is the mistake that returns a worse image rather than an error.
    SwapChannelGroups,
}

/// One region through the three graphs under `reading`, returned as the decoder's raw planar output.
///
/// A second implementation of the sequence, deliberately: the mutations below are not something `restore_region` can
/// be asked for, and a differential check needs to drive the wrong readings as well as the right one.
/// `the_local_sequence_reproduces_the_region_driver` is what holds this to being the same sequence.
fn drive(
    graphs: &[SessionHandle<ort::session::Session>; 3],
    source: &DynamicImage,
    reading: Reading,
) -> (Vec<f32>, Vec<f32>) {
    let plane = LATENT_EDGE * LATENT_EDGE;
    let mut pixels = vec![0.0_f32; PIXELS.len()];
    image_to_chw(&mut pixels, &Sampler::new(source), region(), REGION_EDGE_PX, Normalisation::Signed)
        .expect("the region converts");

    let mut condition = vec![0.0_f32; LATENT.len()];
    Opai::run_graph(&graphs[0], &pixels, PIXELS, &mut condition, LATENT).expect("the encoder runs");

    let mut noise = vec![0.0_f32; LATENT.len()];
    gaussian(&mut noise, 0, 0, NOISE_SEED);

    let mut packed = vec![0.0_f32; PACKED.len()];
    match reading {
        Reading::SwapChannelGroups => pack(&mut packed, &noise, &condition, plane),
        Reading::Correct | Reading::SkipSchedulerStep => pack(&mut packed, &condition, &noise, plane),
    }

    let mut prediction = vec![0.0_f32; LATENT.len()];
    Opai::run_graph(&graphs[1], &packed, PACKED, &mut prediction, LATENT).expect("the transformer runs");

    let mut denoised = vec![0.0_f32; LATENT.len()];
    match reading {
        Reading::SkipSchedulerStep => denoised.copy_from_slice(&prediction),
        Reading::Correct | Reading::SwapChannelGroups => scheduler_step(&mut denoised, &prediction, &noise),
    }

    let mut restored = vec![0.0_f32; PIXELS.len()];
    Opai::run_graph(&graphs[2], &denoised, LATENT, &mut restored, PIXELS).expect("the decoder runs");

    (pixels, restored)
}

/// Task 5.1: one region, end to end, on `provider`.
async fn restores_a_region_on(provider: ExecutionProvider) -> Vec<u8> {
    let opai = application().await;

    assert!(
        opai.providers().supports(provider),
        "this machine reports no support for {provider}; this test needs hardware that has it"
    );

    let source = one_region().await;
    let handles = sessions(&opai, provider).await;
    let graphs = RegionGraphs::<Opai> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };

    let mut scratch = RegionScratch::default();
    let started = std::time::Instant::now();
    let planar = restore_region::<Opai>(&graphs, &mut scratch, &Sampler::new(&source), region())
        .unwrap_or_else(|err| panic!("the region did not restore on {provider}: {err}"));
    let elapsed = started.elapsed();

    // The region hands back the decoder's own planar output — the depth is the pipeline's to apply once, to the
    // finished picture — so the image below is this test's decode rather than the driver's.
    let mut restored = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(REGION_EDGE_PX, REGION_EDGE_PX);
    chw_to_image(&mut restored, planar, Normalisation::Signed).expect("the region decodes");

    assert_eq!(
        restored.dimensions(),
        (REGION_EDGE_PX, REGION_EDGE_PX),
        "the model is resolution-preserving and did not preserve it"
    );

    println!("  one {REGION_EDGE_PX}x{REGION_EDGE_PX} region on {provider} in {elapsed:?}");

    // Kept out of any temporary directory so the result can actually be looked at, which is half of what a first run
    // against a real diffusion model is for.
    let keep = std::env::temp_dir().join(format!("opai-live-osaka-region-{provider}.png"));
    DynamicImage::ImageRgb8(restored.clone()).save(&keep).expect("the region encodes to PNG");
    println!("  inspect it at {}", keep.display());

    restored.into_raw()
}

#[tokio::test]
#[ignore = "needs ~7.2 GB of Osaka FP16 weights and minutes of CPU inference; run by hand with --ignored"]
async fn osaka_restores_a_region_on_the_cpu() {
    restores_a_region_on(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU and ~7.2 GB of Osaka FP16 weights; run by hand on a Mac with --ignored"]
async fn osaka_restores_a_region_on_coreml() {
    restores_a_region_on(ExecutionProvider::CoreMl).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU and ~7.2 GB of Osaka FP16 weights; run by hand on a Mac with --ignored"]
async fn the_cpu_and_coreml_agree_on_the_region_they_produce() {
    // The reference records this graph matching the CPU at cosine 0.99999 or better on CoreML, which is the figure
    // this is held to. A provider that crashes announces itself; one that silently miscomputes does not.
    let cpu = restores_a_region_on(ExecutionProvider::Cpu).await;
    let coreml = restores_a_region_on(ExecutionProvider::CoreMl).await;

    let as_floats = |bytes: &[u8]| bytes.iter().map(|value| f32::from(*value)).collect::<Vec<_>>();
    let similarity = cosine(&as_floats(&cpu), &as_floats(&coreml));

    println!("\n  CPU against CoreML: cosine {similarity:.6}");
    assert!(similarity > 0.9999, "the two providers disagree on this region: cosine {similarity:.6}");
}

#[tokio::test]
#[ignore = "needs ~7.2 GB of Osaka FP16 weights and minutes of CPU inference; run by hand with --ignored"]
async fn the_local_sequence_reproduces_the_region_driver() {
    // What makes the differential test below a test of `region.rs` rather than of a second implementation: driven
    // with no mutation, the local sequence must produce exactly what the driver produces.
    let opai = application().await;
    let source = one_region().await;
    let handles = sessions(&opai, ExecutionProvider::Cpu).await;

    let (_, local) = drive(&handles, &source, Reading::Correct);

    let graphs = RegionGraphs::<Opai> { encoder: &handles[0], transformer: &handles[1], decoder: &handles[2] };
    let mut scratch = RegionScratch::default();
    let driven =
        restore_region::<Opai>(&graphs, &mut scratch, &Sampler::new(&source), region()).expect("the region runs");

    // Both sides are the decoder's own planar output now, so this compares the two sequences directly rather than
    // two quantizations of them — a stricter equality than the 8-bit one it replaced.
    assert_eq!(local.len(), driven.len(), "the two sequences produced different shapes");
    assert_eq!(local, driven, "the local sequence and the region driver disagree");
}

/// Task 5.2: the two readings that can be wrong without failing, checked by differential test.
///
/// Neither is checkable by inspection. Both produce *an image*: skipping the scheduler step decodes the velocity
/// field, and swapping the channel groups feeds the transformer its two latents the wrong way round through a single
/// projection that cannot tell them apart. What distinguishes the right reading from either is that the restored
/// region still carries the structure of the region it was given, which is a number rather than a matter of taste.
///
/// # Measured
///
/// On macOS arm64 against the published FP16 exports on the CPU provider, over `fixtures/test.dat` resampled to
/// 960x960. Pearson correlation of the decoded region against the region the encoder was given:
///
/// ```text
///   as written                   0.981716
///   scheduler step skipped      -0.865825
///   channel groups swapped       0.351622
/// ```
///
/// Neither mutation is a close call, and the first is not merely worse but **negative**: the velocity field is close
/// to the inverse of the image it was derived from, so a pipeline that skipped the step would return an image that
/// is anti-correlated with its input — and still an image, which is why this is a measurement rather than a look.
/// Swapping the two latent groups keeps about a third of the structure, which is the harder of the two to catch by
/// eye: it looks like a poor restoration rather than like a bug.
///
/// The bounds asserted below are deliberately loose against those figures. What they pin is the *ordering* — the
/// reading as written scoring better than either way of getting it wrong — because that is the claim, and a tight
/// bound would fail on a re-export or another provider's arithmetic without anything being wrong.
#[tokio::test]
#[ignore = "needs ~7.2 GB of Osaka FP16 weights and three CPU regions; run by hand with --ignored"]
async fn the_scheduler_step_and_the_channel_order_both_score_better_than_getting_them_wrong() {
    let opai = application().await;
    let source = one_region().await;
    let handles = sessions(&opai, ExecutionProvider::Cpu).await;

    let (input, correct) = drive(&handles, &source, Reading::Correct);
    let correct = correlation(&input, &correct);

    let (_, skipped) = drive(&handles, &source, Reading::SkipSchedulerStep);
    let skipped = correlation(&input, &skipped);

    let (_, swapped) = drive(&handles, &source, Reading::SwapChannelGroups);
    let swapped = correlation(&input, &swapped);

    println!("\n  correlation against the input region");
    println!("    as written                   {correct:.6}");
    println!("    scheduler step skipped       {skipped:.6}");
    println!("    channel groups swapped       {swapped:.6}");

    // The restoration keeps the picture. A reading that did not would not be a worse enhancement, it would be a
    // different image.
    assert!(correct > 0.9, "the region as written does not carry its input's structure: {correct:.6}");

    // And both mutations are *worse*. If either scores the same or better, the reading in `latent.rs` and `region.rs`
    // is wrong and it is those that need revisiting rather than these bounds.
    assert!(
        skipped < correct,
        "skipping the scheduler step scored {skipped:.6} against {correct:.6}: the step is being read wrongly"
    );
    assert!(
        swapped < correct,
        "swapping the channel groups scored {swapped:.6} against {correct:.6}: the packing order is wrong"
    );
}

/// Task 1.1: how the pinned runtime parses `optimization.disable_specified_optimizers`.
///
/// The instrument is Osaka's transformer on the CPU provider: it fails session creation unless
/// `SimplifiedLayerNormFusion` is genuinely switched off, and an unrecognised name is ignored in silence — so a value
/// containing that name either loads, in which case the whole value parsed and every entry in it was read, or fails,
/// in which case it did not. That turns a setting with no readback into a binary observable, and it is why the pairs
/// below are measured rather than the single name: a single name cannot distinguish a separator from a runtime that
/// takes one name at all, which is the mistake this test exists to stop being made twice.
///
/// Measured on an M2 Max against the pinned ONNX Runtime 1.26, through this crate's own session build:
///
/// ```text
///   (unset)                                                        FAIL
///   "SimplifiedLayerNormFusion"                                    LOAD
///   "ReshapeFusion"                                                FAIL
///   "ReshapeFusion,SimplifiedLayerNormFusion"                      FAIL
///   "ReshapeFusion;SimplifiedLayerNormFusion"                      LOAD
///   "SimplifiedLayerNormFusion;ReshapeFusion"                      LOAD
///   "SimplifiedLayerNormFusion;NotATransformerAtAll"               LOAD
///   "ReshapeFusion;SimplifiedLayerNormFusion;NotATransformerAtAll" LOAD
///   "ReshapeFusion; SimplifiedLayerNormFusion"                     FAIL
/// ```
///
/// Three things follow, and the last is the one that is easy to miss. The separator is a **semicolon**, and order
/// does not matter. An unrecognised entry is dropped on its own rather than costing the value — the runtime reads the
/// list, then ignores the names in it that match nothing. And the entries are **not trimmed**: a space after the
/// semicolon makes `" SimplifiedLayerNormFusion"` an entry that matches no transformer, so the graph fails exactly as
/// though nothing had been disabled. That is the whole failure mode in one line — a value that looks right, disables
/// nothing, and says nothing about it.
#[tokio::test]
#[ignore = "needs ~7.2 GB of Osaka FP16 weights and nine CPU session builds; run by hand with --ignored"]
async fn the_disabled_optimizer_setting_is_a_semicolon_separated_list() {
    let opai = application().await;

    let operation =
        Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Fp16), Scale::new(1.0).expect("1.0 is in range"));
    let Resolution::Graphs(set) = operation.resolve() else { panic!("Osaka resolved to passes") };
    let graph = set.resolved(GraphRole::Transformer).expect("Osaka declares a transformer");

    // `true` where the graph must open, which is exactly where the value reached the runtime as a list whose entries
    // include `SimplifiedLayerNormFusion`.
    let cases = [
        (None, false),
        (Some("SimplifiedLayerNormFusion"), true),
        (Some("ReshapeFusion"), false),
        (Some("ReshapeFusion,SimplifiedLayerNormFusion"), false),
        (Some("ReshapeFusion;SimplifiedLayerNormFusion"), true),
        (Some("SimplifiedLayerNormFusion;ReshapeFusion"), true),
        (Some("SimplifiedLayerNormFusion;NotATransformerAtAll"), true),
        (Some("ReshapeFusion;SimplifiedLayerNormFusion;NotATransformerAtAll"), true),
        (Some("ReshapeFusion; SimplifiedLayerNormFusion"), false),
    ];

    println!("\n  optimization.disable_specified_optimizers, Osaka's transformer on the CPU provider");
    for (candidate, expected) in cases {
        // The cache keys on the artifact and the provider rather than on the profile, so without this every candidate
        // after the first would read the first one's session back and report whatever it did.
        opai.release_sessions();

        let profile = EpProfile { disabled_optimizers: entries(candidate), ..graph.profile.clone() };
        let loaded = opai
            .acquire(&graph.artifact, &profile, ExecutionProvider::Cpu, &crate::sessions::Interest::default())
            .await
            .is_ok();

        let shown = candidate.unwrap_or("(unset)");
        println!("    {shown:<62} {}", if loaded { "LOAD" } else { "FAIL" });
        assert_eq!(
            loaded,
            expected,
            "{shown:?} was expected to {}; the runtime no longer parses this setting as it was measured to",
            if expected { "load" } else { "fail" }
        );
    }
}

/// A raw config-entry value as the list a profile carries, so a case above says what reaches the runtime rather than
/// what a model declares.
///
/// Split on the separator this test is measuring, which is the one place in the crate where that is the right thing
/// to do: everywhere else the join is [`DISABLED_OPTIMIZER_SEPARATOR`]'s and the names are never spelled together.
fn entries(value: Option<&str>) -> Vec<String> {
    value
        .map(|value| value.split(DISABLED_OPTIMIZER_SEPARATOR).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Task 6.1: the whole pipeline, through the public API, on the real weights.
///
/// Everything the region tests above answer is about one region. What this answers is the four things only the
/// assembled image can show, and every one of them is a number rather than a look:
///
/// - **It upscales.** The result is the requested dimensions, through `Opai::process` rather than through any
///   internal seam — which is also the end-to-end proof that the refusal in `process::plan_one` is actually lifted.
/// - **It restores rather than resamples.** The result carries the picture it was conditioned on *and* carries more
///   high-frequency detail than that picture has. A pipeline that quietly returned its own base would pass the first
///   and fail the second.
///
///   Task 6.1 worded this as the result correlating with a Lanczos resample of the input more strongly than that
///   resample correlates with the input itself. **That comparison cannot discriminate**, and the first run of this
///   test is what showed it: the input has to be brought to the result's size before it can be correlated at all, so
///   the baseline becomes two interpolations of one photograph, which agree whatever the model did. Measured on
///   macOS arm64 over the CPU provider: the result against its base **0.992214**, the base against an enlargement of
///   the input **0.997828**. No restoration clears that bar.
///
///   What separates the two is detail, not correlation. A Lanczos upsample invents none — it interpolates between
///   the pixels it was given — where a restoration synthesizes texture the interpolation has nowhere to get. So the
///   check is on detail energy, as a strict inequality with no threshold to tune.
/// - **No seam.** No step in value appears across a region boundary that does not appear elsewhere in the picture,
///   which is the property the weighted accumulator exists for.
/// - **No drift.** Each channel's mean tracks the resampled reference's rather than wandering, which is the property
///   the colour fix exists for — and the one a per-region correction would fail while still looking plausible.
///
/// `#[ignore]`d because CI has neither the ~7.2 GB of weights nor the minutes: this is several 960x960 regions on the
/// CPU provider, each of which the region tests above measured at 75 seconds. **Nothing here stubs the model or
/// fabricates an output.** Where the weights cannot be installed the test fails saying so, rather than passing over a
/// stand-in.
#[tokio::test]
#[ignore = "needs ~7.2 GB of Osaka FP16 weights and several CPU regions; run by hand with --ignored"]
async fn osaka_upscales_a_multi_region_image_through_the_public_api() {
    let opai = application().await;

    // Small enough that the run is four regions rather than forty, and large enough that at 2x the target is several
    // regions across — so there are interior seams for the accumulator to have got wrong.
    let source = photograph(700, 520).await;
    let (width, height) = source.dimensions();
    let requested = Scale::new(2.0).expect("2.0 is in range");

    let operation = Operation::Upscale(Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Fp16), requested));

    let reported: std::sync::Arc<std::sync::Mutex<Vec<f64>>> = std::sync::Arc::default();
    let sink = std::sync::Arc::clone(&reported);

    let started = std::time::Instant::now();
    let enhanced = opai
        .process(
            &source,
            std::slice::from_ref(&operation),
            Some(ProcessOptions {
                provider: ExecutionProvider::Cpu,
                // Nothing is read back or left behind, so the timing is the model's rather than a store's.
                cache: false,
                on_progress: Some(std::sync::Arc::new(move |report: &crate::InferenceProgress| {
                    sink.lock().unwrap().push(report.chain_fraction);
                })),
                ..Default::default()
            }),
        )
        .await
        .unwrap_or_else(|err| panic!("the Osaka pipeline did not run through the public API: {err}"));
    let elapsed = started.elapsed();

    let produced = enhanced.picture.pixels();
    let (out_width, out_height) = (produced.width(), produced.height());
    println!("\n  {width}x{height} -> {out_width}x{out_height} at {}x in {elapsed:?}", requested.get());
    println!("  ran on {:?}", enhanced.providers);

    // 1. The requested dimensions, by the same arithmetic every other upscale contract produces.
    assert_eq!(
        (out_width, out_height),
        ((f64::from(width) * 2.0).round() as u32, (f64::from(height) * 2.0).round() as u32),
        "the alignment extension survived into the result, or the crop took the wrong rectangle"
    );

    let bar = reported.lock().unwrap().clone();
    assert_eq!(bar.last().copied(), Some(1.0), "the run did not land on exactly one: {bar:?}");

    // The reference the pipeline conditioned the model on, rebuilt here the same way it builds it.
    let base = source.pixels().resize_exact(out_width, out_height, FilterType::Lanczos3);

    let restored = luminance(produced);
    let resampled = luminance(&base);

    // 2. It restored rather than resampled, which takes two measurements together — see this test's own
    // documentation for why the correlation the task named cannot say it on its own.
    //
    // First: the result still carries the picture it was conditioned on. A run that had diverged would not.
    let to_base = correlation(&restored, &resampled);
    assert!(to_base > 0.9, "the result does not carry the picture it was conditioned on: {to_base:.6}");
    assert_ne!(restored, resampled, "the pipeline handed back its own resample");

    // Second: it carries detail that picture does not have. This is the one that distinguishes a restoration from an
    // interpolation, and it is a strict inequality rather than a tuned bound — a Lanczos upsample synthesizes no
    // detail at all, so a model that restored anything is above it and a pipeline that returned its base is not.
    let restored_detail = detail_energy(&restored, out_width, out_height);
    let base_detail = detail_energy(&resampled, out_width, out_height);
    println!("  correlation to base {to_base:.6}");
    println!("  detail energy: result {restored_detail:.6}, base {base_detail:.6}");

    assert!(
        restored_detail > base_detail,
        "the result carries no more detail than its own resample ({restored_detail:.6} against {base_detail:.6}), \
         so nothing was restored"
    );

    // 3. No seam. The largest step across a column that a region boundary falls on is no larger than the largest step
    // anywhere else in the picture — a seam would be a step the picture itself has nowhere.
    let (seam, elsewhere) = largest_steps(&restored, out_width, out_height);
    println!("  largest step across a seam {seam:.6}, elsewhere {elsewhere:.6}");
    assert!(
        seam <= elsewhere * 1.05,
        "a region boundary shows a step of {seam:.6} against {elsewhere:.6} in the picture at large"
    );

    // 4. No drift. Each channel's mean tracks the reference's — which is the whole of what the colour fix does, and
    // what a per-region correction would fail while still producing a plausible-looking image.
    for channel in 0..3 {
        let produced_mean = channel_mean(produced, channel);
        let reference_mean = channel_mean(&base, channel);

        println!("  channel {channel}: result {produced_mean:.4} against reference {reference_mean:.4}");
        assert!(
            (produced_mean - reference_mean).abs() < 0.02,
            "channel {channel} drifted to {produced_mean:.4} from the reference's {reference_mean:.4}"
        );
    }

    let keep = std::env::temp_dir().join("opai-live-osaka-pipeline.png");
    produced.save(&keep).expect("the result encodes to PNG");
    println!("  inspect it at {}", keep.display());
}

/// The photograph, resampled so that the run is a few regions rather than a few dozen.
///
/// Its own helper rather than [`one_region`] because what this needs is an *image*, at a size the pipeline will
/// itself resample and extend — which is the thing under test.
async fn photograph(width: u32, height: u32) -> Picture {
    let picture = live_support::photograph().await;

    // A fresh identity derived from the source's and what was done to it, as `Picture::new`'s contract requires:
    // these are not the file's pixels any more, and handing back the file's identity would key a cache entry for one
    // picture onto another.
    let resampled = picture.pixels().resize_exact(width, height, FilterType::Lanczos3);
    let identity = rust_sak::crypto::xxh3_string(&format!("{}|{width}x{height}", picture.identity()));

    Picture::new(picture.path(), resampled, identity)
}

/// One image's luminance, as the `[0, 1]` plane every structural comparison above is made over.
///
/// Luminance rather than the three channels, because what those comparisons are about is structure and a per-channel
/// pass would measure the same structure three times.
fn luminance(image: &DynamicImage) -> Vec<f32> {
    let pixels = image.to_rgb8();

    pixels
        .pixels()
        .map(|Rgb([r, g, b])| 0.299 * f32::from(*r) + 0.587 * f32::from(*g) + 0.114 * f32::from(*b))
        .map(|value| value / 255.0)
        .collect()
}

/// How much high-frequency detail a plane carries: the mean absolute Laplacian over its interior.
///
/// The measure that separates a restoration from an interpolation. A Lanczos upsample invents no detail — every value
/// it produces is a weighted combination of pixels the input already had — so its high frequencies are bounded by
/// what the smaller image held. A model that actually restored the image synthesizes texture at the output's own
/// scale, which has nowhere else to come from, and this is that difference as one number.
///
/// The border row and column are skipped rather than clamped: the four-neighbour stencil has no neighbour there, and
/// a clamped edge would report a detail of its own that is the stencil's rather than the picture's.
fn detail_energy(plane: &[f32], width: u32, height: u32) -> f64 {
    let (width, height) = (width as usize, height as usize);
    assert!(width > 2 && height > 2, "a plane this small has no interior to measure");

    let mut total = 0.0_f64;

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let at = y * width + x;
            let laplacian = 4.0 * plane[at] - plane[at - 1] - plane[at + 1] - plane[at - width] - plane[at + width];

            total += f64::from(laplacian.abs());
        }
    }

    total / ((width - 2) * (height - 2)) as f64
}

/// One channel's mean over the whole image, as a `[0, 1]` fraction.
fn channel_mean(image: &DynamicImage, channel: usize) -> f64 {
    let pixels = image.to_rgb8();
    let total: f64 = pixels.pixels().map(|pixel| f64::from(pixel.0[channel]) / 255.0).sum();

    total / pixels.pixels().len() as f64
}

/// The largest step between adjacent columns on a column a region boundary falls on, and the largest anywhere else.
///
/// The seams are where the grid actually put them, derived from the same [`TileGrid`] the driver walks rather than
/// from a figure restated here — so a change to the geometry moves what this inspects rather than silently making it
/// inspect nothing.
fn largest_steps(plane: &[f32], width: u32, height: u32) -> (f32, f32) {
    let (padded_width, padded_height) = padded_extent(width, height);
    let layout =
        TileGrid { size: REGION_EDGE_PX, overlap: REGION_OVERLAP, width: padded_width, height: padded_height }.layout();

    // Every column at which one region ends or the next begins, kept only where it is inside the cropped result.
    let mut seams: Vec<u32> = Vec::new();
    for tile in layout.tiles() {
        for column in [tile.x, tile.x + tile.width] {
            if column > 0 && column < width {
                seams.push(column);
            }
        }
    }
    seams.sort_unstable();
    seams.dedup();

    assert!(!seams.is_empty(), "the geometry produced no interior seam, so this proves nothing");
    println!("  seams at columns {seams:?} of {width}");

    let (mut at_seam, mut elsewhere) = (0.0_f32, 0.0_f32);
    for y in 0..height {
        for x in 1..width {
            let step = (plane[(y * width + x) as usize] - plane[(y * width + x - 1) as usize]).abs();

            if seams.contains(&x) {
                at_seam = at_seam.max(step);
            } else {
                elsewhere = elsewhere.max(step);
            }
        }
    }

    (at_seam, elsewhere)
}
