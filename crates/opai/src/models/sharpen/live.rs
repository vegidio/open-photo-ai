//! The live checks: the real Moscow, Petersburg and Novgorod weights, the real ONNX Runtime, a real photograph, end
//! to end.
//!
//! `#[ignore]`d, as every live check in this crate is. Run them by hand, and **alone** — the guard count they report
//! is process-wide, and the fake-backend suite in `models::filter` trips the guard on purpose:
//!
//! ```text
//! cargo test -p opai models::sharpen::live -- --ignored --nocapture --test-threads=1
//! ```

// The check itself is `live_support::StrengthFamily`'s, shared with denoise: the two families run one pipeline and
// differ here only in their names. Petersburg is the guarded one, and whether its guard fires on a real photograph at
// all is the one claim the fake cannot make.

use crate::live_support::StrengthFamily;
use crate::models::precision::FloatPrecision;
use crate::models::{Operation, Sharpen, SharpenVariant, Strength};
use crate::providers::ExecutionProvider;

/// The family as the shared check drives it.
const SHARPEN: StrengthFamily = StrengthFamily {
    name: "opai-live-sharpen",
    variants: ["moscow", "petersburg", "novgorod"],
    guarded: "petersburg",
    operation,
};

/// The operation running `codename` at `precision` and `strength`.
fn operation(codename: &str, precision: FloatPrecision, strength: Strength) -> Operation {
    let variant = SharpenVariant::from_codename(codename, precision.into())
        .unwrap_or_else(|| panic!("{codename} is not a variant this family publishes"));

    Operation::Sharpen(Sharpen::new(variant, strength))
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and all three variants' weights; run by hand with --ignored"]
async fn every_variant_sharpens_a_photograph_on_the_cpu() {
    SHARPEN.everywhere(ExecutionProvider::Cpu).await;
}

#[tokio::test]
#[ignore = "needs an Apple GPU, and downloads the runtime and all three variants' weights; run by hand on a Mac with --ignored"]
async fn every_variant_sharpens_a_photograph_on_coreml() {
    SHARPEN.everywhere(ExecutionProvider::CoreMl).await;
}
