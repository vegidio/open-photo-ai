//! The live checks: the real Paris and Lyon weights, the real ONNX Runtime, a real photograph, end to end.
//!
//! `#[ignore]`d, as the chain's own live checks and [`osaka::live`](crate::models::upscale::osaka::live) are. Run them
//! by hand:
//!
//! ```text
//! cargo test -p opai models::light_adjustment::live -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The weights are small by this project's standards — Paris at FP16 is 122 KB, the smallest graph published here —
//! so unlike Osaka's these install into a temporary directory and cost nothing to re-run.

// Everything else in this family is arithmetic exercised on every CI platform against a fake backend — the canvas
// plan, the presentation and its mirror, the cropped decode, the gain map, the blend, the progress schedule, the depth
// dispatch, the cancellation and the refusal. What no CI platform can prove is that a **real session produces a
// plausible photograph**, which is the one failure mode this family has: every way this contract can be wrong — a
// truncating fit, a correction applied before the crop, the extension left in the upsample, the wrong normalisation
// range — produces an image and no diagnostic. Each of the three properties `adjusts_on` states fails a *different*
// way of getting it wrong.

use image::DynamicImage;

use imaging::tensor::Sampler;
use rust_sak::image::ImageFormat;

use crate::ProcessOptions;
use crate::live_support::{cropped, green_levels, identical, live_application, photograph};
use crate::models::precision::FloatPrecision;
use crate::models::{Bias, LightAdjustment, Operation};
use crate::providers::ExecutionProvider;

/// The application name these install under.
const NAME: &str = "opai-live-light-adjustment";

/// A bias at each end of the range and at the neutral point, which is the whole of what a run's parameter can be.
const BIASES: [f64; 3] = [-1.0, 0.0, 1.0];

/// The operation running `codename` at `precision` and `bias`.
fn operation(codename: &str, precision: FloatPrecision, bias: f64) -> Operation {
    let bias = Bias::new(bias).expect("a live check supplies a bias in range");

    match codename {
        "paris" => LightAdjustment::paris(precision, bias),
        "lyon" => LightAdjustment::lyon(precision, bias),
        other => panic!("{other} is not a variant this family publishes"),
    }
}

/// The mean of the photograph's green channel, as a stand-in for luminance.
fn mean_level(image: &DynamicImage) -> f64 {
    // Green rather than a weighted sum of the three, because what is being compared is two results of one photograph
    // rather than an absolute measure — any monotone function of brightness separates them, and the green channel
    // carries most of the luminance in every weighting there is.
    let sampler = Sampler::new(image);
    let (width, height) = (image.width(), image.height());

    let total: f64 = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| f64::from(sampler.rgb(x, y)[1]))
        .sum();

    total / (f64::from(width) * f64::from(height))
}

/// Puts the committed photograph through `codename` at `precision` on `provider`, at every bias, and states the
/// three properties.
async fn adjusts_on(codename: &str, precision: FloatPrecision, provider: ExecutionProvider) {
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;
    let (width, height) = source.dimensions();

    println!("\n{codename} at {precision:?} on {provider}: source {width}x{height}");

    let mut results = Vec::with_capacity(BIASES.len());

    for bias in BIASES {
        let options = ProcessOptions { provider, ..Default::default() };
        let started = std::time::Instant::now();

        let enhanced = opai
            .process(&source, &[operation(codename, precision, bias)], Some(options))
            .await
            .unwrap_or_else(|err| panic!("{codename} at {precision:?} on {provider}, bias {bias}: {err}"));

        let produced = enhanced.picture;
        let level = mean_level(produced.pixels());

        println!(
            "  bias {bias:>5}: mean {level:8.2}  in {:?}  on {:?}",
            started.elapsed(),
            enhanced.providers.actual
        );

        // The photograph's own resolution, whatever the square the graph ran at. A run that returned the model's
        // output enlarged would land on the canvas's aspect ratio instead, and one that forgot to drop the extension
        // before the upsample somewhere else again.
        assert_eq!(
            produced.dimensions(),
            (width, height),
            "{codename} at bias {bias} did not return the photograph's own dimensions"
        );

        results.push((bias, produced, level));
    }

    // A bias of zero returns the photograph unchanged — after the graph has run, the crop has happened and the gain
    // map has been applied, all of which are then multiplied by nothing, so anything that reached the pixels by
    // another route shows here.
    let (_, neutral, _) = &results[1];
    assert!(
        identical(neutral.pixels(), source.pixels()),
        "{codename} at a bias of 0 did not return the photograph unchanged"
    );

    // And the two ends move it in opposite directions from where it started: the extrapolation is the one thing about
    // the bias that is not visible in the arithmetic of a single run.
    let before = mean_level(source.pixels());
    let (negative, positive) = (results[0].2, results[2].2);

    println!("  photograph {before:.2}, at -1 {negative:.2}, at +1 {positive:.2}");
    assert_ne!(
        (positive - before).signum(),
        (negative - before).signum(),
        "{codename}: a bias of -1 and of +1 moved mean luminance the same way, {negative:.2} and {positive:.2} \
         against the photograph's {before:.2}"
    );

    // Kept where it can be looked at, which is half of what a first run against real weights is for: a wrong
    // normalisation or a fit off by a pixel is a photograph rather than a number.
    for (bias, produced, _) in &results {
        let keep = std::env::temp_dir().join(format!("opai-live-{codename}-{precision:?}-{provider}-b{bias}.png"));

        crate::image::save(produced.shared_pixels(), &keep, ImageFormat::Png, None)
            .await
            .expect("a result must encode to PNG");
        println!("  inspect bias {bias} at {}", keep.display());
    }
}

/// `provider` against both precisions of both variants; the tests below call it once for each of the two providers
/// this project's hardware has.
async fn adjusts_everywhere(provider: ExecutionProvider) {
    for codename in ["paris", "lyon"] {
        for precision in FloatPrecision::ALL {
            adjusts_on(codename, precision, provider).await;
        }
    }
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and both variants' weights; run by hand with --ignored"]
async fn both_variants_adjust_a_photograph_on_the_cpu() {
    adjusts_everywhere(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU, and downloads the runtime and both variants' weights; run by hand on a Mac with --ignored"]
async fn both_variants_adjust_a_photograph_on_coreml() {
    adjusts_everywhere(ExecutionProvider::CoreMl).await;
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and both variants' weights; run by hand with --ignored"]
async fn a_photograph_that_is_not_square_keeps_its_own_aspect_ratio_through_the_extension() {
    // **The committed fixture is square**, so the checks above never reach the two steps that only exist for a
    // photograph that is not: the reflection that fills the canvas, and the cropped decode that takes it back out.
    // Both are where the extension can leak into the result, and neither announces itself — what a reader would see
    // is a photograph whose right edge came from its own mirror.
    //
    // So this one crops the fixture to a landscape and a portrait and puts each through real weights. What it
    // states is not a pixel value but the property the crop exists for: the result is the photograph's own shape,
    // and at a bias of zero it is the photograph.
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;

    for (width, height) in [(600_u32, 300_u32), (300, 600), (640, 137)] {
        let cropped = cropped(&source, width, height);

        for codename in ["paris", "lyon"] {
            let enhanced = opai
                .process(&cropped, &[operation(codename, FloatPrecision::Fp32, 0.0)], None)
                .await
                .unwrap_or_else(|err| panic!("{codename} over a {width}x{height} photograph: {err}"));

            assert_eq!(
                enhanced.picture.dimensions(),
                (width, height),
                "{codename} did not return a {width}x{height} photograph's own dimensions"
            );

            // And the extension did not reach any of it, which on a 640x137 source is 79% of the canvas being a
            // mirror of the picture. At a bias of zero the whole chain runs and is then multiplied by nothing, so
            // anything the crop let through would be a pixel that is not the photograph's.
            assert!(
                identical(enhanced.picture.pixels(), cropped.pixels()),
                "{codename} over a {width}x{height} photograph did not return it unchanged at a bias of 0"
            );

            println!("  {codename}: {width}x{height} came back at its own shape, unchanged at a bias of 0");
        }
    }
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and Paris' weights; run by hand with --ignored"]
async fn a_sixteen_bit_run_carries_sixteen_bits_through_the_gain_map() {
    // The divergence from the reference, against real weights: it forces every intermediate to 8-bit RGBA, so a
    // 16-bit photograph would have its gain map computed from 256 levels and that answer written into a 16-bit
    // result. The fake-backend suite checks the dispatch; this checks that a real graph's output survives it.
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;

    let options = ProcessOptions { depth: crate::OutputDepth::Sixteen, ..Default::default() };
    let bias = Bias::new(1.0).expect("1.0 is in range");

    let enhanced = opai
        .process(&source, &[LightAdjustment::paris(FloatPrecision::Fp32, bias)], Some(options))
        .await
        .expect("a 16-bit Paris run");

    let produced = enhanced.picture;
    assert!(
        matches!(produced.pixels(), DynamicImage::ImageRgb16(_)),
        "a 16-bit request did not produce a 16-bit result"
    );

    // And it carries more than an 8-bit channel could have held.
    let levels = green_levels(produced.pixels());

    println!("a 16-bit Paris run carries {levels} distinct green levels");
    assert!(levels > 256, "a 16-bit result carried only {levels} distinct levels");
}
