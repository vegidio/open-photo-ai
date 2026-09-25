//! The live checks: this family's real weights, the real ONNX Runtime, a real photograph, end to end.
//!
//! Everything else in this family is arithmetic exercised on every CI platform against a fake backend — the canvas
//! plan, the presentation and its mirror, the cropped sample reader, the solver, the full-resolution apply, the
//! blend, the progress schedule, the depth dispatch, the cancellation and the zero-area refusal. What no CI platform can
//! prove is that a **real session produces a plausible photograph**, which is the one failure mode this family has:
//! every way this contract can be wrong — a truncating fit, the extension left in the samples, the source and the
//! destination swapped, a channel read from the wrong plane, the polynomial evaluated in the wrong feature order —
//! produces an image and no diagnostic.
//!
//! # Why they are `#[ignore]`d
//!
//! They download the pinned runtime and this family's weights, and CoreML needs an Apple GPU no CI runner has. The same
//! arrangement, for the same reason, as the chain's own live checks and
//! [`light_adjustment::live`](crate::models::light_adjustment). Run them by hand:
//!
//! ```text
//! cargo test -p opai models::color_balance::live -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Rio's graph is small by this project's standards and São Paulo's is under 34 MB, so these install into a
//! temporary directory and cost nothing to re-run.
//!
//! # What each check states
//!
//! Three properties shared by both variants, and each one fails a *different* way of getting a contract here wrong:
//!
//! - **The result carries the photograph's own dimensions.** A pipeline that returned the model's output enlarged,
//!   or that fitted the mapping and then applied it to the low-resolution square, lands somewhere else.
//! - **A bias of 0 returns the photograph unchanged.** The whole chain — present, run, crop, fit, apply, blend — is
//!   exercised and then multiplied by zero, so anything that reached the pixels by another route shows here.
//! - **A positive and a negative bias move mean chromaticity in opposite directions.** The extrapolation is the one
//!   thing about the bias that is not visible in the arithmetic of a single run, and chromaticity rather than
//!   luminance because what this family moves is a cast.
//!
//! And a fourth that is **São Paulo's rather than the family's**: the result's *luminance structure* tracks the
//! photograph's. Half of that model's graph output is shown rather than sampled, so a weight plane read as a
//! rendering, a rendering read as a weight, or the two renderings swapped against their weight planes each
//! produces an image — with correctly loaded weights and nothing anywhere to say so. Every one of them destroys
//! the correspondence between where the photograph is light and where the result is, which no amount of colour
//! casting does.

use image::DynamicImage;

use rust_sak::image::ImageFormat;

use crate::ProcessOptions;
use crate::live_support::{cropped, green_levels, identical, live_application, photograph};
use crate::models::precision::FloatPrecision;
use crate::models::{Bias, ColorBalance, Operation};
use crate::providers::ExecutionProvider;
use imaging::tensor::Sampler;

/// The application name these install under.
const NAME: &str = "opai-live-color-balance";

/// A bias at each end of the range and at the neutral point, which is the whole of what a run's parameter can be.
const BIASES: [f64; 3] = [-1.0, 0.0, 1.0];

/// One of this family's models, as the checks below name it: the display name a failure reports, the codename the
/// inspection files are written under, the constructor that builds a run of it, and whether its graph output is
/// **shown** rather than only sampled into a fit — which is what the luminance-structure check is asked of.
///
/// Two spellings rather than one lower-cased on the spot, for the reason the variant table already carries both:
/// São Paulo's label has an accent and a space, and a path composed from it is a file name no two of this project's
/// three platforms would escape the same way.
type Model = (&'static str, &'static str, fn(FloatPrecision, Bias) -> Operation, bool);

/// The two models this family publishes.
///
/// A table rather than two copies of the body below, because the three properties every check states are the
/// **family's** and neither model is entitled to a weaker version of them. What is São Paulo's alone is the fourth,
/// and that one is asked for by name.
const MODELS: [Model; 2] = [
    ("Rio", "rio", ColorBalance::rio, false),
    ("São Paulo", "saopaulo", ColorBalance::saopaulo, true),
];

/// The operation running `model` at `precision` and `bias`.
fn operation(model: fn(FloatPrecision, Bias) -> Operation, precision: FloatPrecision, bias: f64) -> Operation {
    model(precision, Bias::new(bias).expect("a live check supplies a bias in range"))
}

/// The Rec. 709 luminance of one pixel, in `[0, 1]`.
fn luma(rgb: [u16; 3]) -> f64 {
    let [r, g, b] = rgb.map(|value| f64::from(value) / f64::from(u16::MAX));

    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// How closely `produced`'s luminance tracks `source`'s, as a Pearson correlation over every pixel.
///
/// **São Paulo's own check, and it exists because half of that model's graph output is *shown* rather than sampled
/// into a fit.** A weight plane read as a rendering, a rendering read as a weight, or the two renderings swapped
/// against their weight planes each produces an image from correctly loaded weights, with nothing anywhere to say
/// so — and each destroys the correspondence between where the photograph is light and where the result is.
///
/// A correlation rather than a difference, because the correction is *allowed* to move luminance: every one of the
/// three renderings is a white balance, so a strong cast shifts the mean and the spread. What it may not do is
/// stop the bright parts of the result being the bright parts of the photograph.
fn luminance_correlation(source: &DynamicImage, produced: &DynamicImage) -> f64 {
    let (width, height) = (source.width(), source.height());
    let (source, produced) = (Sampler::new(source), Sampler::new(produced));

    let pixels: Vec<(f64, f64)> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| (luma(source.rgb(x, y)), luma(produced.rgb(x, y))))
        .collect();

    let count = pixels.len() as f64;
    let mean_left = pixels.iter().map(|(left, _)| left).sum::<f64>() / count;
    let mean_right = pixels.iter().map(|(_, right)| right).sum::<f64>() / count;

    let mut covariance = 0.0;
    let mut left_variance = 0.0;
    let mut right_variance = 0.0;

    for (left, right) in pixels {
        let (left, right) = (left - mean_left, right - mean_right);

        covariance += left * right;
        left_variance += left * left;
        right_variance += right * right;
    }

    covariance / (left_variance * right_variance).sqrt()
}

/// The photograph's mean red-minus-blue, as a stand-in for where its colour sits between warm and cool.
///
/// Chromaticity rather than luminance, which is what [`light_adjustment`](crate::models::light_adjustment)'s own
/// live check measures: a white balance moves the picture along the warm-cool axis and can leave its mean
/// brightness almost where it was, so a luminance measure would separate the two ends of the bias only by accident.
///
/// Red minus blue rather than a proper chromaticity coordinate, because what is being compared is two results of
/// one photograph rather than an absolute measure — any monotone function of the cast separates them, and this one
/// needs no division and no guard against a black pixel.
fn warmth(image: &DynamicImage) -> f64 {
    let sampler = Sampler::new(image);
    let (width, height) = (image.width(), image.height());

    let total: f64 = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let [r, _, b] = sampler.rgb(x, y);
            f64::from(r) - f64::from(b)
        })
        .sum();

    total / (f64::from(width) * f64::from(height))
}

/// Puts the committed photograph through `model` at `precision` on `provider`, at every bias, and states the
/// properties.
async fn balances_on((name, codename, model, shown): Model, precision: FloatPrecision, provider: ExecutionProvider) {
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;
    let (width, height) = source.dimensions();

    println!("\n{name} at {precision:?} on {provider}: source {width}x{height}");

    let mut results = Vec::with_capacity(BIASES.len());

    for bias in BIASES {
        let options = ProcessOptions { provider, ..Default::default() };
        let started = std::time::Instant::now();

        let enhanced = opai
            .process(&source, &[operation(model, precision, bias)], Some(options))
            .await
            .unwrap_or_else(|err| panic!("{name} at {precision:?} on {provider}, bias {bias}: {err}"));

        let produced = enhanced.picture;
        let level = warmth(produced.pixels());

        println!(
            "  bias {bias:>5}: warmth {level:8.2}  in {:?}  on {:?}",
            started.elapsed(),
            enhanced.providers.actual
        );

        // The photograph's own resolution, whatever the square the graph ran at. A run that returned the model's
        // output enlarged would land on the canvas's aspect ratio instead, and one that applied the mapping to the
        // square rather than to the photograph would land on 656.
        assert_eq!(
            produced.dimensions(),
            (width, height),
            "{name} at bias {bias} did not return the photograph's own dimensions"
        );

        results.push((bias, produced, level));
    }

    // A bias of zero returns the photograph unchanged — after the graph has run, the extension has been cropped
    // out and the mapping has been fitted and evaluated, all of which are then multiplied by nothing.
    let (_, neutral, _) = &results[1];
    assert!(
        identical(neutral.pixels(), source.pixels()),
        "{name} at a bias of 0 did not return the photograph unchanged"
    );

    // And the two ends move the colour in opposite directions from where it started.
    let before = warmth(source.pixels());
    let (negative, positive) = (results[0].2, results[2].2);

    println!("  photograph {before:.2}, at -1 {negative:.2}, at +1 {positive:.2}");
    assert_ne!(
        (positive - before).signum(),
        (negative - before).signum(),
        "{name}: a bias of -1 and of +1 moved mean chromaticity the same way, {negative:.2} and {positive:.2} \
         against the photograph's {before:.2}"
    );

    // The fourth property, and **São Paulo's rather than the family's**: the fully corrected result's luminance
    // structure still tracks the photograph's. Asked at a bias of 1, where the whole of the model's contribution
    // has landed, and of this model alone because it is the only one here whose graph output is shown rather than
    // sampled — the mistakes it guards against (a weight plane read as a rendering, the renderings swapped against
    // their planes) are not mistakes Rio's single-output contract can make.
    if shown {
        let (_, corrected, _) = &results[2];
        let correlation = luminance_correlation(source.pixels(), corrected.pixels());

        println!("  luminance correlation with the photograph {correlation:.4}");
        assert!(
            correlation > 0.99,
            "{name}'s result tracks the photograph's luminance at only {correlation:.4}, which is what a weight \
             plane read as a rendering looks like"
        );
    }

    // Kept where it can be looked at, which is half of what a first run against real weights is for: a wrong
    // normalisation or a fit taken over the extension is a photograph rather than a number.
    for (bias, produced, _) in &results {
        let keep = std::env::temp_dir().join(format!("opai-live-{codename}-{precision:?}-{provider}-b{bias}.png"));

        crate::image::save(produced.shared_pixels(), &keep, ImageFormat::Png, None)
            .await
            .expect("a result must encode to PNG");
        println!("  inspect bias {bias} at {}", keep.display());
    }
}

/// Both variants at both precisions, which is every operation this family publishes.
async fn balances_everywhere(provider: ExecutionProvider) {
    for model in MODELS {
        for precision in FloatPrecision::ALL {
            balances_on(model, precision, provider).await;
        }
    }
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and this family's weights; run by hand with --ignored"]
async fn both_variants_balance_a_photograph_on_the_cpu() {
    balances_everywhere(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU, and downloads the runtime and this family's weights; run by hand on a Mac with --ignored"]
async fn both_variants_balance_a_photograph_on_coreml() {
    balances_everywhere(ExecutionProvider::CoreMl).await;
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and this family's weights; run by hand with --ignored"]
async fn a_photograph_that_is_not_square_keeps_its_own_aspect_ratio_through_the_extension() {
    // **The committed fixture is square**, so the checks above never reach the two steps that only exist for a
    // photograph that is not: the reflection that fills the canvas, and the crop that keeps it out of the fit.
    // Both are where the extension can reach the result, and neither announces itself — what a reader would see is
    // a photograph corrected slightly wrongly, which is the 4.8 dB the crop is worth.
    //
    // So this one crops the fixture to a landscape and a portrait and puts each through real weights. What it
    // states is not a pixel value but the property the crop exists for: the result is the photograph's own shape,
    // and at a bias of zero it is the photograph.
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;

    // Both variants, because the extension reaches São Paulo's contract twice over: the crop keeps it out of the
    // two fits, as it does Rio's one, **and** the weight map is read only over the region the photograph occupies.
    // A map read across the whole square is stretched against the photograph it lines up with, which on a 640x137
    // source displaces every weight in it by most of the canvas.
    for (name, _, model, _) in MODELS {
        for (width, height) in [(600_u32, 300_u32), (300, 600), (640, 137)] {
            // The **model** is not in the crop's identity, because these are one photograph run twice rather than
            // two photographs: what keeps the two runs apart in the cache is the operation's own tag, which carries
            // the codename.
            let cropped = cropped(&source, width, height);

            let enhanced = opai
                .process(&cropped, &[operation(model, FloatPrecision::Fp32, 0.0)], None)
                .await
                .unwrap_or_else(|err| panic!("{name} over a {width}x{height} photograph: {err}"));

            assert_eq!(
                enhanced.picture.dimensions(),
                (width, height),
                "{name} did not return a {width}x{height} photograph's own dimensions"
            );

            // And nothing the extension contributed reached the result. On a 640x137 source the extension is 79% of
            // the canvas, so a fit polluted by it is a visibly different correction — which at a bias of zero is
            // multiplied by nothing, leaving this as the check that nothing bypassed the bias at all.
            assert!(
                identical(enhanced.picture.pixels(), cropped.pixels()),
                "{name} over a {width}x{height} photograph did not return it unchanged at a bias of 0"
            );

            println!("  {name}: {width}x{height} came back at its own shape, unchanged at a bias of 0");
        }
    }
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and this family's weights; run by hand with --ignored"]
async fn a_sixteen_bit_run_carries_sixteen_bits_through_the_mapping() {
    // The divergence from the reference, against real weights: both of its pipelines render into an 8-bit RGBA
    // buffer, so a 16-bit photograph would have the polynomials written out at 256 levels per channel. The
    // fake-backend suites check the dispatch; this checks that a real graph's fit survives it, for both contracts.
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;

    for (name, _, model, _) in MODELS {
        let options = ProcessOptions { depth: crate::OutputDepth::Sixteen, ..Default::default() };

        let enhanced = opai
            .process(&source, &[operation(model, FloatPrecision::Fp32, 1.0)], Some(options))
            .await
            .unwrap_or_else(|err| panic!("a 16-bit {name} run: {err}"));

        let produced = enhanced.picture;
        assert!(
            matches!(produced.pixels(), DynamicImage::ImageRgb16(_)),
            "{name}: a 16-bit request did not produce a 16-bit result"
        );

        // And it carries more than an 8-bit channel could have held.
        let levels = green_levels(produced.pixels());

        println!("a 16-bit {name} run carries {levels} distinct green levels");
        assert!(levels > 256, "{name}: a 16-bit result carried only {levels} distinct levels");
    }
}
