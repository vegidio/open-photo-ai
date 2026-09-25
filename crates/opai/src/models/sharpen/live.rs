//! The live checks: the real Moscow, Petersburg and Novgorod weights, the real ONNX Runtime, a real photograph, end
//! to end.
//!
//! `#[ignore]`d, as every live check in this crate is. Run them by hand, and **alone** — the guard count they report
//! is process-wide, and the fake-backend suite in `models::filter` trips the guard on purpose:
//!
//! ```text
//! cargo test -p opai models::sharpen::live -- --ignored --nocapture --test-threads=1
//! ```

// Everything else in this family is exercised on every CI platform against a fake backend — the tile grid at scale 1,
// the guard, the blend, the progress schedule, the depth dispatch, the cancellation and the refusal. What no CI
// platform can prove is that a real session produces a photograph at its own size, that the strength gates every route
// to the pixels, and whether Petersburg's guard fires on a real photograph at all — which is the one claim the fake
// cannot make.

use std::sync::atomic::Ordering;

use rust_sak::image::ImageFormat;

use crate::models::filter::GUARDED;

use crate::ProcessOptions;
use crate::live_support::{identical, live_application, photograph};
use crate::models::precision::FloatPrecision;
use crate::models::{Operation, Sharpen, SharpenVariant, Strength};
use crate::providers::ExecutionProvider;

/// The application name these install under.
const NAME: &str = "opai-live-sharpen";

/// The operation running `codename` at `precision` and `strength`.
fn operation(codename: &str, precision: FloatPrecision, strength: f64) -> Operation {
    let strength = Strength::new(strength).expect("a live check supplies a strength in range");

    let variant = SharpenVariant::from_codename(codename, precision.into())
        .unwrap_or_else(|| panic!("{codename} is not a variant this family publishes"));

    Operation::Sharpen(Sharpen::new(variant, strength))
}

/// Puts the committed photograph through `codename` at `precision` on `provider`, at a strength of 0 and of 1, states
/// the properties, and hands back the result at 1.
async fn sharpens_on(codename: &str, precision: FloatPrecision, provider: ExecutionProvider) -> crate::Picture {
    // A fresh application per call, so no result is served from another provider's run of the same operation.
    let (_root, opai) = live_application(NAME).await;
    let source = photograph().await;
    let (width, height) = source.dimensions();

    println!("\n{codename} at {precision:?} on {provider}: source {width}x{height}");

    let mut sharpened = None;

    for strength in [1.0, 0.0] {
        let options = ProcessOptions { provider, ..Default::default() };
        let guarded_before = GUARDED.load(Ordering::Relaxed);
        let started = std::time::Instant::now();

        let enhanced = opai
            .process(&source, &[operation(codename, precision, strength)], Some(options))
            .await
            .unwrap_or_else(|err| panic!("{codename} at {precision:?} on {provider}, strength {strength}: {err}"));

        let guarded = GUARDED.load(Ordering::Relaxed) - guarded_before;
        let produced = enhanced.picture;

        println!(
            "  strength {strength}: {:?} on {:?}, {guarded} tile(s) kept by the guard",
            started.elapsed(),
            enhanced.providers.actual
        );

        assert_eq!(
            produced.dimensions(),
            (width, height),
            "{codename} at strength {strength} did not return the photograph's own dimensions"
        );

        if codename != "petersburg" {
            assert_eq!(guarded, 0, "{codename} is not guarded, yet a tile was kept");
        }

        if strength == 0.0 {
            // After every tile has run and been stitched, and then multiplied by nothing: anything that reached the
            // pixels by another route shows here.
            assert!(
                identical(produced.pixels(), source.pixels()),
                "{codename} at a strength of 0 did not return the photograph unchanged"
            );
        } else {
            assert!(
                !identical(produced.pixels(), source.pixels()),
                "{codename} at a strength of 1 returned the photograph untouched"
            );

            // Kept where it can be looked at: a wrong normalisation or a seam is a photograph rather than a number.
            let keep =
                std::env::temp_dir().join(format!("opai-live-{codename}-{precision:?}-{provider}-t{strength}.png"));

            crate::image::save(produced.shared_pixels(), &keep, ImageFormat::Png, None)
                .await
                .expect("a result must encode to PNG");
            println!("  inspect at {}", keep.display());

            sharpened = Some(produced);
        }
    }

    sharpened.expect("the run at a strength of 1 keeps its result")
}

/// `provider` against both precisions of all three variants, and the three against one another at each precision.
async fn sharpens_everywhere(provider: ExecutionProvider) {
    const VARIANTS: [&str; 3] = ["moscow", "petersburg", "novgorod"];

    // A delta rather than a reset, as each run's own count is, so the total below is this check's own even in a
    // process that ran something else guarded before it.
    let guarded_before = GUARDED.load(Ordering::Relaxed);

    for precision in FloatPrecision::ALL {
        let mut results = Vec::with_capacity(VARIANTS.len());

        for codename in VARIANTS {
            results.push(sharpens_on(codename, precision, provider).await);
        }

        // Three sets of weights at one strength: two identical results would mean a seam handed two variants one graph.
        for (left, right) in [(0, 1), (0, 2), (1, 2)] {
            assert!(
                !identical(results[left].pixels(), results[right].pixels()),
                "{} and {} at {precision:?} on {provider} produced the same photograph",
                VARIANTS[left],
                VARIANTS[right]
            );
        }
    }

    println!(
        "\n{provider}: {} tile(s) kept by Petersburg's guard across both precisions",
        GUARDED.load(Ordering::Relaxed) - guarded_before
    );
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and all three variants' weights; run by hand with --ignored"]
async fn every_variant_sharpens_a_photograph_on_the_cpu() {
    sharpens_everywhere(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU, and downloads the runtime and all three variants' weights; run by hand on a Mac with --ignored"]
async fn every_variant_sharpens_a_photograph_on_coreml() {
    sharpens_everywhere(ExecutionProvider::CoreMl).await;
}
