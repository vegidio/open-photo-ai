//! The live checks: a real photograph, a real ONNX Runtime, a real model, end to end.
//!
//! Everything else in this module is exercised against a fake per-tile function on every CI platform — the pass
//! sequence, the correcting resample, all three progress weightings, the depth resolution, the identity composition
//! and every refusal. What cannot be is the one thing that needs a runtime: feeding a tensor to ONNX Runtime and
//! getting a photograph back.
//!
//! Most of them run Kyoto end to end through [`crate::Opai::process`]. The last two run **Tokyo**, one tile at a
//! time through the session directly, because what they check is a property of the runtime's own scheduling that an
//! end-to-end run cannot vary: that a model declaring sequential execution produces the pixels the parallel one
//! does.
//!
//! # Why it is `#[ignore]`d
//!
//! It downloads the pinned ONNX Runtime — ~175 MB on the platforms that ship it with its execution providers — and
//! then Kyoto's weights, and the whole of the rest of this repository's suite is hermetic. The same arrangement, for
//! the same reason, as [`crate::sessions`]'s four live checks and [`crate::runtime`]'s. Run it by hand:
//!
//! ```text
//! cargo test -p opai -- --ignored --nocapture
//! ```
//!
//! # It enhances a photograph, not a gradient
//!
//! `fixtures/test.dat` at the workspace root, carried over unchanged from the Go application so a measurement taken
//! here can be compared against one taken there. A synthetic gradient would answer none of the questions this test
//! exists to answer: the seam blend already has a proxy assertion against synthetic tiles, and what it has never been
//! checked against is real detail, which is where a hard edge down a tile boundary would actually show.
//!
//! `OPAI_LIVE_PHOTO` overrides it with a photograph of your own, which is how a larger one is put through the 8x run
//! below when what is wanted is the memory figure for a full-size image rather than for a 640x640 one.
//!
//! # What running it answered
//!
//! Run by hand on **macOS arm64 (Apple Silicon)**, against `fixtures/test.dat` — 640x640 — with the pinned ONNX
//! Runtime 1.26.0 (`git-commit-id=8c546c37b4`) and Kyoto FP16. **The timings below are from a `dev` build**, so they
//! are useful for comparing one provider against another and useless as absolute figures; nothing here was measured
//! under `--release`.
//!
//! **Slice 2's option maps produce sessions that actually compute, on both providers.** Kyoto 4x, 640x640 to
//! 2560x2560: **62.5 s on the CPU, 9.0 s on CoreML** — a 6.5x margin, and the first time anything in this project has
//! run inference at all. CoreML emits its usual `VerifyEachNodeIsAssignedToAnEp` warning about shape-related ops
//! staying on the CPU, which is expected and is not a partition failure.
//!
//! **The seam blend leaves no visible edge on a photograph**, which is the question slice 4's D11 left open and could
//! not answer against synthetic tiles. At 640 wide the grid's offsets are `[0, 240, 384]`, so a 4x pass puts its
//! seams at x=960 and x=1536. Over the 2560x2560 result the mean absolute difference between adjacent columns is
//! **1.86**; the two seams measure **2.03 and 2.43**, ranking 833rd and 623rd of 2559 boundaries. The worst boundary
//! in the whole image is **37.95 at x=1282** — an edge in the photograph, not a tile join. The rows behave the same
//! way, the two seams ranking 471st and 776th. Looked at as well as measured: a crop centred on the x=960/y=960
//! intersection shows the gradients running straight through it.
//!
//! **An `Rgb16` result survives both encoders.** The 8x 16-bit run produced 5120x5120 `Rgb16` as asked and encoded to
//! **266,268 bytes of AVIF and 726,954 bytes of HEIF** — a path `rust-sak` supports and this project had never
//! exercised.
//!
//! **What an 8x 16-bit run costs, and it is worse than the source size suggests.** 640x640 to 5120x5120 in 24.2 s
//! over two passes, the result alone holding 150 MB of pixels — and the process peaking at **1.72 GB of resident
//! memory**, against 357 MB for the 4x 8-bit run. Part of that is the three encodes of a 150 MB image that follow,
//! but the shape of it is the point: **a 0.4-megapixel source reaches 1.7 GB at 8x.** The risk the design records
//! under that name is real and understated — a 24-megapixel photograph is nowhere near servable at 8x, and nothing
//! budgets it. If the memory budget deferred past this series is ever to move up the queue, this is the number that
//! argues for it.
//!
//! **The NVIDIA providers remain unverified.** Nothing in this project has run inference on CUDA or TensorRT, on any
//! machine, ever. That stays true until somebody with the hardware runs the CUDA case below, and this says so rather
//! than implying coverage that does not exist.

use crate::cache::CacheMode;
use crate::image::ImageFormat;
use crate::inference::depth::OutputDepth;
use crate::inference::process::ProcessOptions;
use crate::live_support::{live_application, photograph};
use crate::models::{FloatPrecision, Operation, Scale, Upscale, UpscaleVariant};
use crate::pipeline::session::run_tile;
use crate::providers::ExecutionProvider;
use crate::providers::profile::{EpProfile, ExecutionMode};
use imaging::grid::Tile;
use imaging::tensor::{Normalisation, Sampler, image_to_chw};

/// How a result's pixels are described in the output below, so a reader sees `Rgb16` rather than a discriminant.
fn describe(pixels: &image::DynamicImage) -> &'static str {
    match pixels {
        image::DynamicImage::ImageRgb8(_) => "Rgb8",
        image::DynamicImage::ImageRgb16(_) => "Rgb16",
        _ => "something the pipeline does not produce",
    }
}

/// The application name these install under, which is also the configuration directory they create.
const NAME: &str = "opai-live-inference-test";

/// A Kyoto operation at `scale`, at FP16 — the precision whose provider profile is the measured one.
fn kyoto(scale: f64) -> Operation {
    Operation::Upscale(Upscale::new(
        UpscaleVariant::Kyoto(FloatPrecision::Fp16),
        Scale::new(scale).expect("the test asked for a scale in range"),
    ))
}

/// A Tokyo operation at `scale`, at FP16 — the one variant in the library whose profile declares an execution mode.
fn tokyo(scale: f64) -> Operation {
    Operation::Upscale(Upscale::new(
        UpscaleVariant::Tokyo(FloatPrecision::Fp16),
        Scale::new(scale).expect("the test asked for a scale in range"),
    ))
}

/// Upscales the photograph 4x on `provider` and checks what came back.
async fn upscales_on(provider: ExecutionProvider) {
    let (_root, opai) = live_application(NAME).await;

    assert!(
        opai.providers().supports(provider),
        "this machine reports no support for {provider}; this test needs hardware that has it"
    );

    let source = photograph().await;
    let (width, height) = source.dimensions();

    let options = ProcessOptions { provider, depth: OutputDepth::Source, ..Default::default() };
    let started = std::time::Instant::now();

    let enhanced = opai
        .process(&source, &[kyoto(4.0)], Some(options))
        .await
        .unwrap_or_else(|err| panic!("Kyoto did not run on {provider}: {err}"));

    let result = &enhanced.picture;

    assert_eq!(result.dimensions(), (width * 4, height * 4), "the result is not four times the source");
    assert_ne!(result.identity(), source.identity(), "the result carried the source's identity");

    // The one place in the suite where the report describes a **real** provider rather than a fake's answer. A live
    // run built exactly one session, so it names exactly one provider — and whether that is the one asked for is the
    // whole point: on a machine that cannot serve `provider`, the timing printed below is a CPU timing and the line
    // above it is what says so.
    assert_eq!(enhanced.providers.requested, provider, "the report lost what was asked for");
    assert_eq!(
        enhanced.providers.actual.len(),
        1,
        "a one-pass run built other than one session: {:?}",
        enhanced.providers
    );

    println!(
        "Kyoto 4x on {provider} (ran on {:?}): {width}x{height} -> {}x{} at {} in {:?}",
        enhanced.providers.actual,
        result.dimensions().0,
        result.dimensions().1,
        describe(result.pixels()),
        started.elapsed()
    );
    println!("  {}", ort::info());

    // Kept out of the temporary directory so the seam can actually be looked at, which is half of what this run is
    // for: a hard edge would fall down the last column and along the last row, where the grid moved a tile back.
    let keep = std::env::temp_dir().join(format!("opai-live-kyoto-4x-{provider}.png"));
    crate::image::save(result.shared_pixels(), &keep, ImageFormat::Png, None)
        .await
        .expect("a result must encode to PNG");
    println!("  inspect the seam at {}", keep.display());
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and Kyoto's weights; run by hand with --ignored"]
async fn kyoto_upscales_a_photograph_on_the_cpu() {
    upscales_on(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU, and downloads the runtime and Kyoto's weights; run by hand on a Mac with --ignored"]
async fn kyoto_upscales_a_photograph_on_coreml() {
    upscales_on(ExecutionProvider::CoreMl).await;
}

#[tokio::test]
#[ignore = "needs an NVIDIA card, which nothing in this project has ever run inference on; run by hand with --ignored"]
async fn kyoto_upscales_a_photograph_on_cuda() {
    upscales_on(ExecutionProvider::Cuda).await;
}

/// A 16-bit result through AVIF and HEIF, which `rust-sak` supports and this project had never exercised, and the
/// memory an 8x 16-bit run costs.
///
/// 8x is two passes — Kyoto's 4x weights and then its 2x — over an image that is sixteen times larger on the second,
/// so this is also where the seam is worth looking at: the last column and the last row of the second pass are the
/// ones the grid moved back, and the ones a narrow ramp would leave an edge on.
#[tokio::test]
#[ignore = "downloads the pinned runtime and Kyoto's weights; run by hand with --ignored"]
async fn an_eight_times_sixteen_bit_result_survives_the_avif_and_heif_encoders() {
    let (root, opai) = live_application(NAME).await;
    let source = photograph().await;
    let (width, height) = source.dimensions();

    let options = ProcessOptions { depth: OutputDepth::Sixteen, ..Default::default() };
    let started = std::time::Instant::now();

    let enhanced = opai
        .process(&source, &[kyoto(8.0)], Some(options))
        .await
        .unwrap_or_else(|err| panic!("an 8x 16-bit Kyoto run failed: {err}"));

    // Two passes, so two handles — and a provider that opened both is named once rather than twice, which is the one
    // property of the report that a two-pass live run checks and a single-pass one cannot.
    let result = &enhanced.picture;
    assert!(
        enhanced.providers.actual.len() <= 2,
        "an 8x run named more providers than it built sessions on: {:?}",
        enhanced.providers
    );

    assert_eq!(result.dimensions(), (width * 8, height * 8));
    assert!(
        matches!(result.pixels(), image::DynamicImage::ImageRgb16(_)),
        "a 16-bit request produced {}",
        describe(result.pixels())
    );

    println!(
        "Kyoto 8x at 16 bits: {width}x{height} -> {}x{} in {:?}",
        width * 8,
        height * 8,
        started.elapsed()
    );
    // What the result alone holds, which is the floor under the run's peak rather than the peak itself: the source,
    // the pass's input and output, and the driver's scratch are all live alongside it while it is being produced.
    println!(
        "  the result holds {} MB of pixels",
        (width as u64 * 8) * (height as u64 * 8) * 3 * 2 / 1_048_576
    );

    // Both encoders, at 16 bits, over the real result — which is the path that had never been exercised.
    for (format, extension) in [(ImageFormat::Avif, "avif"), (ImageFormat::Heif, "heif")] {
        let destination = root.path().join(format!("upscaled.{extension}"));
        let written = crate::image::save(result.shared_pixels(), &destination, format, None)
            .await
            .unwrap_or_else(|err| panic!("a 16-bit result did not encode to {extension}: {err}"));

        assert!(written > 0, "the {extension} encoder wrote nothing");
        println!("  {extension}: {written} bytes at {}", destination.display());
    }

    // Kept out of the temporary directory so the seam can actually be looked at, which is what this run is for.
    let keep = std::env::temp_dir().join("opai-live-kyoto-8x.png");
    crate::image::save(result.shared_pixels(), &keep, ImageFormat::Png, None)
        .await
        .expect("a 16-bit result must encode to PNG");
    println!("  inspect the seam at {}", keep.display());
}

/// What the run cache costs on the way in and saves on the way back, at both depths.
///
/// **A measurement rather than a correctness test.** Everything the cache is required to *do* — that a hit is the
/// picture the run produced, that a second identical run acquires no session and runs no tile, that the depth
/// distinguishes two entries, that a failed write does not fail a run — is a checked property in
/// [`process`](super::process) and [`cache`](crate::cache), against a fake backend on every CI platform. Nothing here
/// asserts a threshold, because a timing on one machine in one build profile is not a contract.
///
/// What it answers is the one question the design left to a number rather than to an argument: the PNG encode and the
/// write are on the run's critical path, and `PngCompression::Fast` is the mitigation carried from the reference
/// without anybody having measured it here. **If the write turns out to cost a real fraction of the run it saves, this
/// is what justifies revisiting the stored format** — which is local to two functions in `cache.rs` and changes
/// nothing that calls them.
///
/// Each depth is run four times against a store of its own:
///
/// 0. with the cache off and **untimed**, which builds the session. Without it the first timed run carries a session
///    build the others do not, and the subtraction below measures the provider's compiler rather than the encode.
/// 1. with the cache off, which is the model alone;
/// 2. with the cache on and empty, which is the model plus the encode and the write;
/// 3. with the cache on and warm, which is the decode of a stored entry.
///
/// # What running it answered
///
/// Run by hand on **macOS arm64 (Apple Silicon)**, against `fixtures/test.dat` — 640x640 — with the pinned ONNX
/// Runtime 1.26.0, Kyoto FP16 and `ExecutionProvider::Auto`, which resolves to CoreML here. Taken twice, and **the
/// build profile turned out to be the whole answer**, so both are recorded: every other figure in this module is from
/// a `dev` build, and reading this one that way would have condemned the stored format on an artefact.
///
/// | | `dev`, 8-bit | `dev`, 16-bit | `--release`, 8-bit | `--release`, 16-bit |
/// |---|---|---|---|---|
/// | the model alone | 2.75 s | 2.71 s | 1.56 s | 1.56 s |
/// | the model, encoded and stored | 5.25 s | 9.29 s | 1.61 s | 1.67 s |
/// | **what the write cost** | 2.50 s | 6.58 s | **55 ms** | **118 ms** |
/// | **as a share of the run it saves** | 91% | 243% | **3.5%** | **7.6%** |
/// | a hit | 0.45 s | 1.35 s | 0.054 s | 0.103 s |
///
/// **D3 needs no revisiting: `PngCompression::Fast` costs a few percent of the run it saves.** The `dev` figures say
/// the write costs as much as the enhancement and at 16 bits more than twice as much, which would have been the
/// argument for a cheaper stored format — and they are wrong about the shipped application. `miniz_oxide`'s deflate is
/// exactly the kind of tight loop an unoptimized build punishes, and the model, which runs inside ONNX Runtime's own
/// optimized library either way, does not move between the two profiles. Optimized, the encode is 55 ms at 8 bits and
/// 118 ms at 16 against a 1.56 s run.
///
/// **A hit costs 29x less than the run at 8 bits and 15x less at 16**, which is what the slice was for.
///
/// What this does **not** answer is the shape at scale. This is a 0.4-megapixel source; the 8x 16-bit run measured
/// above produces a 5120x5120 result, sixteen times these pixels, and the encode's cost and its second large buffer
/// both grow with it. That belongs with the memory risk this module already records rather than with the format
/// choice, and if it ever does bite, D3's escape is local: a cheaper lossless encoding is two functions in
/// `cache.rs` and no change to anything that calls them.
///
#[tokio::test]
#[ignore = "measurement, not a correctness check; downloads the runtime and Kyoto's weights — run by hand with --ignored"]
async fn what_the_cache_costs_to_fill_and_saves_when_it_is_warm() {
    for depth in [OutputDepth::Eight, OutputDepth::Sixteen] {
        // A store of its own per depth, so the second depth's cold run is actually cold: the two are different
        // entries by construction, but a fresh application is what makes that visible rather than assumed.
        let (_root, opai) = live_application(NAME).await;
        assert_eq!(opai.cache_mode(), CacheMode::Disk, "this measurement needs a real disk store");

        let source = photograph().await;
        let (width, height) = source.dimensions();
        let chain = [kyoto(4.0)];

        // Untimed, so that what the timed runs differ by is the cache rather than a session this one had to build.
        // The store is still empty afterwards, because this run neither read from it nor wrote to it.
        let warm_up = ProcessOptions { depth, cache: false, ..Default::default() };
        opai.process(&source, &chain, Some(warm_up)).await.expect("the warm-up run must succeed");

        let uncached = ProcessOptions { depth, cache: false, ..Default::default() };
        let started = std::time::Instant::now();
        let alone = opai
            .process(&source, &chain, Some(uncached))
            .await
            .expect("an uncached run must succeed")
            .picture;
        let model_only = started.elapsed();

        let cold = ProcessOptions { depth, ..Default::default() };
        let started = std::time::Instant::now();
        let stored = opai.process(&source, &chain, Some(cold)).await.expect("a cold cached run must succeed").picture;
        let with_write = started.elapsed();

        let warm = ProcessOptions { depth, ..Default::default() };
        let started = std::time::Instant::now();
        let served = opai.process(&source, &chain, Some(warm)).await.expect("a warm run must succeed");
        let hit = started.elapsed();

        // The run every operation of which was served from the store built no session, so it names none — on real
        // hardware, where "it ran on what was asked for" would have been the plausible wrong answer.
        assert!(
            served.providers.actual.is_empty(),
            "a fully-served run claimed to have executed on {:?}",
            served.providers.actual
        );

        let served = served.picture;

        // The measurement is only worth reading if the three runs produced one picture, so this much is asserted.
        assert_eq!(served.identity(), stored.identity());
        assert_eq!(served.identity(), alone.identity(), "the bypass produced a different picture");
        assert_eq!(served.pixels().as_bytes(), stored.pixels().as_bytes(), "the hit is not what was stored");

        println!(
            "Kyoto 4x at {depth:?}: {width}x{height} -> {}x{} at {}",
            served.dimensions().0,
            served.dimensions().1,
            describe(served.pixels())
        );
        println!("  the model alone:            {model_only:?}");
        println!("  the model, encoded, stored: {with_write:?}");
        println!("  what the write cost:        {:?}", with_write.saturating_sub(model_only));
        println!("  a hit:                      {hit:?}");
    }
}

/// A real enhancement, through the real sink and the real formatter, read back as a person would read `opai.log`.
///
/// **The check the instrumentation sweep is for**, and it is here rather than in `cli` for the reason the change
/// that added these records gives: neither front end has a processing surface yet — `cli` prints a version — so this
/// is the only place in the project where a real photograph goes through a real ONNX Runtime with a sink installed.
/// When `cli` grows a processing command, the equivalent walkthrough moves there.
///
/// What it asserts is the *account*: the pair of records bracketing the run, the model build with the provider it
/// landed on, and — the reason the levels were chosen the way they were — nothing per tile. It prints the file, so
/// running it by hand answers "does this read as an account of the run" for a person as well as for the assertions.
#[tokio::test]
#[ignore = "downloads the pinned runtime and Kyoto's weights; run by hand with --ignored"]
async fn a_real_run_leaves_an_account_of_itself_in_the_log() {
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;

    let (log, result) =
        crate::logging::records_of("info", || async { opai.process(&source, &[kyoto(4.0)], None).await }).await;
    let result = result.expect("a live Kyoto 4x run must succeed");

    println!("--- opai.log, as a reader would find it ---\n{log}");

    // The bracket, at the default level: what was asked for on the way in, how long it took on the way out.
    let opening = log.lines().find(|line| line.contains(r#"msg="enhancement started""#)).expect(&log);
    assert!(opening.contains(&format!("identity={}", source.identity())), "{opening}");
    assert!(opening.contains("operations=1"), "{opening}");

    let closing = log.lines().find(|line| line.contains(r#"msg="enhancement finished""#)).expect(&log);
    assert!(closing.contains("duration="), "{closing}");

    // The model was built, and the record says on what — which is the answer to "why was this slow" and the whole
    // reason the downgrade beside it is a warning.
    let ready = log.lines().find(|line| line.contains(r#"msg="session ready""#)).expect(&log);
    assert!(ready.contains("artifact=up_kyoto_4x_fp16"), "{ready}");
    assert!(ready.contains("provider="), "{ready}");
    assert!(ready.contains("duration="), "{ready}");

    // And nothing per tile. A 640x640 source at 4x is several tiles per pass, and the whole default-level file is a
    // handful of lines — which is the volume requirement, seen on the real path rather than against a fake backend.
    let records = log.lines().filter(|line| line.contains("msg=")).count();
    assert!(records < 20, "a single run wrote {records} records at the default level:\n{log}");
    assert!(!log.contains("upscale pass finished"), "a debug record reached the default level:\n{log}");

    assert_eq!(result.picture.dimensions(), (source.dimensions().0 * 4, source.dimensions().1 * 4));
}

/// The claim `ExecutionMode::Sequential` rests on: that it is a **setting** rather than a trade.
///
/// `execution-providers` requires that a declaration of sequential execution not change the pixels the model
/// produces, which is what lets the profile take the CUDA win without anyone weighing an accuracy cost against it.
/// Nothing else in the suite can check it — every other assertion about the mode reads the option map, and the
/// option map cannot say what the runtime then computes. So this runs the same graph twice, once each way, and
/// compares the floats the runtime returned.
///
/// **Tokyo rather than Kyoto**, because Tokyo is the variant that declares the mode and because its 2682 nodes are
/// where an inter-op pool would have the most opportunity to reorder anything. One tile rather than a whole image:
/// what is in question is the runtime's scheduling of one `Run`, and the tiling above it is arithmetic that has
/// nothing to do with the mode.
///
/// The two sessions are built one after the other with a release between them, because the session cache is keyed on
/// the artifact alone — a second request for the same graph would otherwise be served the first session, profile and
/// all, and the test would compare a run against itself.
///
/// # What running it answered
///
/// Run by hand on **macOS arm64 (Apple Silicon)** against the pinned ONNX Runtime 1.26.0
/// (`git-commit-id=8c546c37b4`), Tokyo FP16, resolved to CoreML: **all 3,145,728 floats are bit-identical between
/// the two modes**, reproduced across two runs. The CPU sibling below answered the same way on the same tile, so
/// the requirement holds on both providers this project can reach and the profile's `Sequential` is a setting
/// rather than a trade on each of them.
///
/// **The two timings the test prints are not a comparison of the modes, and should not be read as one.** They were
/// 38.8 s and 8.9 s, and again 39.1 s and 9.0 s — an asymmetry that is the on-disk compiled-model cache rather than
/// the setting. Each run gets a fresh configuration directory, so the first session compiles the graph for the
/// device and the second, after `release_sessions`, loads what the first left in `ModelCacheDirectory`. Whichever
/// mode went first would be the slow one. A real comparison would have to warm the cache before timing either, and
/// it is not what this test is for: `models::upscale::tokyo::profile` carries the margins, measured under `--release`
/// through `perftest`.
///
/// It is also worth saying which provider answered what. CoreML takes all 2682 nodes as one partition, so there is
/// no inter-op scheduling left for it to do, and this is the case least likely to have differed — which is why it is
/// the weaker half of the pair. The CPU provider is where the mode actually changes what the runtime does, and the
/// sibling below is the one that establishes the requirement there; it came back bit-identical too, at 18.5 s
/// sequential against 13.8 s parallel.
///
/// That CPU pair is 34% the *other* way from the -24.2% `models::upscale::tokyo::profile` records for this graph, and it is
/// not a refutation of it. The profile's figure is a median over repeated timed runs of a whole 640x640 image
/// through `perftest`; this is one 256x256 tile with an uncached session build folded into it, taken once. What
/// these two tests are for is the pixels — anything about speed belongs to the harness that controls for the build.
#[tokio::test]
#[ignore = "downloads the pinned runtime and Tokyo's weights; run by hand with --ignored"]
async fn a_sequential_model_produces_the_pixels_the_parallel_one_does_on_coreml() {
    same_pixels_either_way(ExecutionProvider::CoreMl).await;
}

/// The same check on the CPU provider, which is where the mode is not a formality.
///
/// Separated rather than folded into the one above because the two answer different questions: CoreML proves the
/// requirement on the provider a Mac actually runs, and this proves it on the one where ONNX Runtime has an inter-op
/// pool to schedule against in the first place. Its outcome and its timings are recorded with that test's, since
/// neither is worth reading without the other.
#[tokio::test]
#[ignore = "downloads the pinned runtime and Tokyo's weights, and runs a SwinIR tile on the CPU; run by hand with --ignored"]
async fn a_sequential_model_produces_the_pixels_the_parallel_one_does_on_the_cpu() {
    same_pixels_either_way(ExecutionProvider::Cpu).await;
}

/// Runs one Tokyo tile on `provider` in each execution mode and compares the floats that came back.
async fn same_pixels_either_way(provider: ExecutionProvider) {
    let (_root, opai) = live_application(NAME).await;

    assert!(
        opai.providers().supports(provider) || provider == ExecutionProvider::Cpu,
        "this machine reports no support for {provider}; this test needs hardware that has it"
    );

    let operation = tokyo(4.0);
    let artifact = operation.required_artifacts().remove(0);

    let sequential = operation.profile();
    assert_eq!(
        sequential.execution_mode,
        ExecutionMode::Sequential,
        "this test exists because Tokyo declares the mode; it no longer does"
    );
    let parallel = EpProfile { execution_mode: ExecutionMode::Parallel, ..sequential.clone() };

    // One tile off the top-left of the photograph, at the geometry the driver would have partitioned it at.
    let shape = imaging::TileGeometry::default().size;
    let plane = (shape as usize) * (shape as usize);
    let source = photograph().await;
    let mut input = vec![0.0f32; 3 * plane];
    image_to_chw(
        &mut input,
        &Sampler::new(source.pixels()),
        Tile { x: 0, y: 0, width: shape, height: shape },
        shape,
        Normalisation::Unit,
    )
    .expect("the scratch is exactly three planes of the tile shape");

    // 4x on each side, which is sixteen times the floats.
    let mut from_sequential = vec![0.0f32; 3 * plane * 16];
    let mut from_parallel = vec![0.0f32; from_sequential.len()];

    let started = std::time::Instant::now();
    let handle = opai
        .session(&artifact, &sequential, provider, &crate::sessions::Interest::default())
        .await
        .unwrap_or_else(|err| panic!("{artifact} did not open sequentially: {err}"));
    run_tile(&handle, &input, &mut from_sequential).expect("the sequential session must run the tile");
    let ran_on = handle.provider();
    let sequential_took = started.elapsed();

    // The cache is keyed on the artifact, so the second session only exists once the first is gone.
    drop(handle);
    opai.release_sessions();

    let started = std::time::Instant::now();
    let handle = opai
        .session(&artifact, &parallel, provider, &crate::sessions::Interest::default())
        .await
        .unwrap_or_else(|err| panic!("{artifact} did not open in parallel: {err}"));
    run_tile(&handle, &input, &mut from_parallel).expect("the parallel session must run the tile");
    let parallel_took = started.elapsed();

    let differing = from_sequential
        .iter()
        .zip(&from_parallel)
        .filter(|(sequential, parallel)| sequential.to_bits() != parallel.to_bits())
        .count();
    let worst = from_sequential
        .iter()
        .zip(&from_parallel)
        .map(|(sequential, parallel)| (sequential - parallel).abs())
        .fold(0.0f32, f32::max);

    assert_eq!(
        differing,
        0,
        "{differing} of {} floats differ between the two modes, by up to {worst}; the mode is a trade rather than a \
         setting and Tokyo's profile takes an accuracy cost it does not account for",
        from_sequential.len()
    );

    println!(
        "Tokyo 4x on {ran_on}, one {shape}x{shape} tile: {} floats bit-identical between the modes \
         (sequential {sequential_took:?}, parallel {parallel_took:?})",
        from_sequential.len()
    );
    println!("  {}", ort::info());
}
