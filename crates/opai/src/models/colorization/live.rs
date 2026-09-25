//! The live checks: the real Delhi, Mumbai and Jaipur weights, the real ONNX Runtime, a real photograph, end to end.
//!
//! `#[ignore]`d, as every live check in this crate is. Run them by hand:
//!
//! ```text
//! cargo test -p opai models::colorization::live -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The six graphs total about 4.0 GB, so they install into a directory that is kept between runs (see
//! [`kept_dir`]) rather than a temporary one. **On CoreML, Mumbai's first run can include a compile of about five
//! minutes**, which the reference records. A long first run there is that compile, not a hang. (The first run made
//! here, on an M2 Max on 2026-09-23, took 8 s; why it did not compile for five minutes was not investigated.)

// Everything else in this family is exercised on every CI platform against a fake backend: the contract dispatch, the
// shapes, the stretch, the compose, the depth, the transparency, the progress schedule, the cancellation and the
// refusal. What no CI platform can show is that a real graph's output, read through its own contract, puts colour onto
// the photograph while leaving its lightness alone, and that the three variants are three different models.
//
// The timings printed are for the record. They are not a measurement, and no profile is derived from them.

use std::time::{Duration, Instant};

use image::{DynamicImage, ImageBuffer, Rgb};

use super::lab;

use crate::live_support::{identical, kept_dir, live_application_at, photograph};
use crate::models::precision::FloatPrecision;
use crate::models::{Colorization, ColorizationVariant, Operation};
use crate::providers::ExecutionProvider;
use crate::{Opai, Picture, ProcessOptions};
use imaging::tensor::{Channel, Sampler};

/// The application name these install under, and the directory they keep.
const NAME: &str = "opai-live-colorization";

/// The operation running `codename` at `precision`.
fn operation(codename: &str, precision: FloatPrecision) -> Operation {
    let variant = ColorizationVariant::from_codename(codename, precision.into())
        .unwrap_or_else(|| panic!("{codename} is not a variant this family publishes"));

    Operation::Colorization(Colorization::new(variant))
}

/// The committed photograph reduced to its gray, so that any colour in a result came from the model.
async fn gray_photograph() -> Picture {
    let source = photograph().await;
    let sampler = Sampler::new(source.pixels());
    let (width, height) = source.dimensions();

    let gray = ImageBuffer::from_fn(width, height, |x, y| Rgb([u8::from_unit(lab::gray(sampler.rgb(x, y))); 3]));

    Picture::new(source.path(), DynamicImage::ImageRgb8(gray), format!("{}-gray", source.identity()))
}

/// The mean absolute difference in Lab L between `result` and `source`, and the mean Lab chroma of `result`.
fn lightness_and_chroma(result: &DynamicImage, source: &DynamicImage) -> (f64, f64) {
    let (width, height) = (result.width(), result.height());
    let (result, source) = (Sampler::new(result), Sampler::new(source));

    let (mut drift, mut chroma) = (0.0_f64, 0.0_f64);
    for y in 0..height {
        for x in 0..width {
            let pixel = result.rgb(x, y);

            drift += f64::from((lab::lightness(pixel) - lab::lightness(source.rgb(x, y))).abs());

            let [_, a, b] = lab::rgb_to_lab(pixel.map(|value| f32::from(value) / 65535.0));
            chroma += f64::from(a.hypot(b));
        }
    }

    let count = f64::from(width) * f64::from(height);
    (drift / count, chroma / count)
}

/// Colorizes the gray photograph through `codename` at `precision` on `provider`, twice, states the properties, and
/// hands back the result.
async fn colorizes_on(
    opai: &Opai,
    source: &Picture,
    codename: &str,
    precision: FloatPrecision,
    provider: ExecutionProvider,
) -> Picture {
    let (width, height) = source.dimensions();
    let mut times: Vec<Duration> = Vec::with_capacity(2);
    let mut result = None;

    // Twice, with the run cache off, so the second is a warm session rather than a cached image and the first carries
    // the session build and any compile.
    for _ in 0..2 {
        let options = ProcessOptions { provider, cache: false, ..Default::default() };
        let started = Instant::now();

        let enhanced = opai
            .process(source, &[operation(codename, precision)], Some(options))
            .await
            .unwrap_or_else(|err| panic!("{codename} at {precision:?} on {provider}: {err}"));

        times.push(started.elapsed());
        result = Some(enhanced);
    }

    let enhanced = result.expect("two runs were made");
    let produced = enhanced.picture;
    let (drift, chroma) = lightness_and_chroma(produced.pixels(), source.pixels());

    let actual = enhanced.providers.actual;
    println!("  {codename:>6} {precision:?} on {actual:?}: cold {:?}, warm {:?}", times[0], times[1]);
    println!("         lightness drift {drift:.3} L, mean chroma {chroma:.2}");

    assert_eq!(
        produced.dimensions(),
        (width, height),
        "{codename} did not return the photograph's own dimensions"
    );

    // Under one L unit on average: the result's lightness is the photograph's own, at full resolution. A model output
    // enlarged in place of the compose would carry the square's blur and miss this by far more.
    assert!(
        drift < 1.0,
        "{codename} at {precision:?} moved the photograph's lightness by {drift} L on average"
    );

    // The gray input carries none, so colour above 2 on average came from the model. An all-zero or unread output
    // would leave this at zero.
    assert!(
        chroma > 2.0,
        "{codename} at {precision:?} put no colour onto the photograph (mean chroma {chroma})"
    );

    // Kept where it can be looked at: a swapped plane or a wrong normalisation is a photograph rather than a number.
    let keep = std::env::temp_dir().join(format!("opai-live-{codename}-{precision:?}-{provider}.png"));
    crate::image::save(produced.shared_pixels(), &keep, rust_sak::image::ImageFormat::Png, None)
        .await
        .expect("a result must encode to PNG");
    println!("         inspect at {}", keep.display());

    produced
}

/// `provider` against both precisions of all three variants, and the three against one another at each precision.
async fn colorizes_everywhere(provider: ExecutionProvider) {
    const VARIANTS: [&str; 3] = ["delhi", "mumbai", "jaipur"];

    let opai = live_application_at(NAME, &kept_dir(NAME)).await;
    let source = gray_photograph().await;
    let (width, height) = source.dimensions();

    println!("\n{provider}: gray source {width}x{height}");

    for precision in FloatPrecision::ALL {
        let mut results = Vec::with_capacity(VARIANTS.len());

        for codename in VARIANTS {
            results.push(colorizes_on(&opai, &source, codename, precision, provider).await);
        }

        // Three sets of weights over one photograph: two identical results would mean a seam handed two variants one
        // graph.
        for (left, right) in [(0, 1), (0, 2), (1, 2)] {
            assert!(
                !identical(results[left].pixels(), results[right].pixels()),
                "{} and {} at {precision:?} on {provider} produced the same photograph",
                VARIANTS[left],
                VARIANTS[right]
            );
        }
    }
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and all three variants' weights (4.0 GB); run by hand with --ignored"]
async fn every_variant_colorizes_a_photograph_on_the_cpu() {
    colorizes_everywhere(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU, and downloads the runtime and all three variants' weights (4.0 GB); run by hand on a Mac with --ignored"]
async fn every_variant_colorizes_a_photograph_on_coreml() {
    colorizes_everywhere(ExecutionProvider::CoreMl).await;
}
