//! Paris: the IAT exposure model a front end offers first — the square its graph was exported at and the
//! execution-provider profile measured for it. The adjustment it runs is the family's [`process`](super::process).

// Deleting the model is deleting this directory plus the arms in `LightAdjustmentVariant` that name it.
//
// Which weights these are: IAT (arXiv 2205.14871), the `IAT_enhance` variant, from the `best_Epoch_exposure.pth`
// checkpoint rather than the `best_Epoch_lol_v1.pth` one the same release ships. Nothing in the original export
// records which, so it is identified by matching the graph's initializers against both: 67 of the 81 tensors with 50
// or more elements are bit-identical to the exposure checkpoint and none at all to `lol_v1`. The 14 unmatched are the
// `conv_large` BatchNorms, folded into their convolutions at export. That agrees with the upstream `img_demo.py`,
// where `--task exposure` loads exactly that file.
//
// Why the graph is a quarter of the size the architecture implies: IAT's local branch is six `CBlock_ln` transformer
// blocks. In the published weights — **both** checkpoints, so this is a property of the release rather than of a bad
// download — every one of those blocks has its `Aff_channel` (alpha, beta, color), `conv1`, `conv2`, `attn` and `mlp`
// tensors stored as float32 denormals around 4e-41. They underflow to zero the moment they multiply anything, so each
// block's two residual branches contribute either exactly 0 (the multiplicative ones) or a per-channel constant of at
// most 2.3e-4 (the additive ones, whose gammas survived). What is left of a block is `x + pos_embed(x)`, a depthwise
// 3x3.
//
// So the export folds the stack down to what it computes: one 3x3 lift, three depthwise 3x3 convolutions per path
// with the constants folded into their biases, and the two end convolutions. Measured against the unfolded graph at
// four resolutions that is 136-140 dB — float round-off rather than an approximation — and it is 78 nodes and 280 KB
// where the unfolded graph is 320 and 410 KB.
//
// **The folding is specific to these weights.** Retrain IAT, or fine-tune from a checkpoint whose blocks are alive,
// and it is wrong: the move then is to re-export the unfolded architecture, not to patch this one.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile, ExecutionMode};

// Why the canvas is a fixed square: a graph with dynamic spatial axes and a dynamic batch does not run on CoreML at
// all. With static input shapes required — which is what a model declaring no profile gets — the provider declines
// all but 4 of its 272 nodes and the rest run on CPU kernels; with the requirement off the session fails to build
// outright (*"axis 4 is not in valid range [-4,3]"*). The tell is that such a graph lands on the same figure through
// CoreML as through the CPU provider in both precisions, around 640 ms at 1024x688.
//
// No provider setting reaches that, so the fixed shape is not a tuning choice but the only way onto the GPU. At a
// fixed square the folded graph is a single CoreML partition — at FP32; FP16 adds only the trailing FP32 output
// `Cast`, as Lyon's does — and the same sweep gives about 10 ms in both precisions. That is a 98% cut, and
// understated, since the square is 1.49x the pixels of the row it is compared against. The CPU provider gains too,
// 646 ms to 88 ms, which is what makes this the right shape on Windows and Linux as well rather than a Mac-only trade.
//
// Why 1024: a fixed square is not output-neutral, and the reason is **not** the padding. IAT's global branch reduces
// over the whole canvas to produce one gamma and one colour matrix, so its answer moves with the scale it is shown:
// the same 640x640 image fed at 1024 shifts the result by +7.6 levels with no padding involved at all. Reflect, wrap,
// edge and whole-image pad fills all measured within noise of each other, so the reflection stays as it is.
//
// That makes the size the whole decision, and it is settled by staying near the scale the dynamic-axes graph renders
// at: over 27 crops above its 1024 ceiling, 1024 holds 41.9 dB median against what that graph renders, where 768
// manages 31.2 dB and 512 27.3 dB. 1024 wins because the dynamic-axes graph resizes those images to it as well; only
// the extension is new.
//
// **Images below that ceiling change more, and that is the real cost of this.** The dynamic-axes graph runs them at
// their own size, and the fixed square resamples them onto it like everything else.
/// The square this graph accepts, and the only one it accepts.
pub(crate) const CANVAS: u32 = 1024;

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// and sequential execution at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Transcribed from the reference's `paris.go` and re-confirmed on this project's own sweep. Everything below is an
    // M2 Max against ONNX Runtime 1.26 at the 1024 square.
    //
    // CoreML off the Neural Engine, at FP16 only: left to choose, CoreML's default compute units cost roughly **twice
    // what the CPU and GPU alone do** — 18.3 ms against 9.2 ms — and the restriction is also what turns this model's
    // FP16 graph from slower than its FP32 one into a tie with it. The Neural Engine loses in both precisions and loses
    // badly: +872% at FP32 and +72% at FP16. Lyon reaches the same answer on the same hardware, measured separately —
    // see `LightAdjustmentVariant::profile` — which is why both files carry their own numbers rather than one citing
    // the other.
    //
    // Sequential execution, at FP16 only: a much smaller figure than the compute units, and declared on the strength
    // of its **consistency** rather than its size: -2.3% and -2.9% on CoreML across two build orders, -0.4% on the CPU
    // provider, and the lowest minimum of any configuration measured for this graph.
    //
    // Neither is declared at FP32. The sequential mode measures +1.2% on CoreML there against -2.7% on the CPU
    // provider, which is a wash rather than a setting — and a declaration that is free at one precision and worth
    // something at the other is not worth carrying to a precision that did not earn it. That is the precision-split
    // rule Saitama and Kyoto already follow, reached here from a different measurement. The reference needs an
    // `Fp16Only` helper to enforce by hand what a match over the precision states directly.
    //
    // CoreML's latency specialization hint and its low-precision GPU accumulation both measured within 0.8% in both
    // precisions for this graph and are not set — carried by the `..EpProfile::default()` below rather than named, so
    // what the test pins is that nothing else was named either.
    //
    // Re-confirmed with `perftest paris --precision <p> -p <provider> -n 20`, on an M2 Max (64 GB, macOS 26.6) against
    // the pinned ONNX Runtime 1.26, over the embedded 640x640 sample. These are **whole-pipeline** medians — the
    // presentation resample, the graph, two full-resolution resamples and two full-resolution loops — where the
    // reference's figures above are of the graph alone:
    //
    //   paris, median of 20 runs      CoreML      CPU provider
    //     FP32                         78.2ms         147.2ms
    //     FP16                         80.3ms         155.4ms
    //
    // **The direction and the size both reproduce.** The two precisions are within 2.7% of each other on CoreML,
    // which is the *tie* the compute-unit restriction is credited with producing — without it FP16 is the slower of
    // the two. And the pipeline around the graph costs about 70 ms on this image whatever ran, which is confirmed
    // independently by `lyon`'s own sweep: subtracting it leaves roughly 9 ms on CoreML against the reference's 9.2,
    // and roughly 77 ms on the CPU provider against its 88.
    //
    // The graph compiles as **one CoreML partition** at both precisions — 76 of 77 nodes at FP16, 75 of 75 at FP32 —
    // which is what the fixed square is for, and is checked the same way Lyon's precondition is.
    match precision {
        Precision::Fp16 => EpProfile {
            coreml_compute_units: CoreMlComputeUnits::CpuAndGpu,
            execution_mode: ExecutionMode::Sequential,
            ..EpProfile::default()
        },
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::light_adjustment::LightAdjustmentVariant;
    use crate::models::precision::FloatPrecision;

    #[test]
    fn paris_carries_exactly_the_two_measured_settings_at_fp16() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break: nothing else asserts the arm reaches this file, and a profile that had been left at
        // the default would still load, still adjust the right photograph, and cost twice as much on an M2 Max.
        assert_eq!(
            LightAdjustmentVariant::Paris(FloatPrecision::Fp16).profile(),
            EpProfile {
                coreml_compute_units: CoreMlComputeUnits::CpuAndGpu,
                execution_mode: ExecutionMode::Sequential,
                ..EpProfile::default()
            },
            "Paris at FP16 is not the pair of settings measured for it"
        );
    }

    #[test]
    fn paris_declares_nothing_at_fp32() {
        // The other half, and the claim rather than a formality: carrying the FP16 answer across would be applying a
        // measurement to a precision that did not earn it — see `profile`'s FP32 figures.
        assert_eq!(
            LightAdjustmentVariant::Paris(FloatPrecision::Fp32).profile(),
            EpProfile::default(),
            "Paris at FP32 declared a setting nothing measured"
        );
    }

    #[test]
    fn paris_runs_at_one_square_and_it_is_the_one_the_graph_was_exported_at() {
        // Pinned as a literal because it is not a tunable: the whole of the canvas-size argument above was taken at
        // this number, and a graph that accepts one size accepts this one.
        assert_eq!(CANVAS, 1024);

        for precision in FloatPrecision::ALL {
            assert_eq!(LightAdjustmentVariant::Paris(precision).canvas(), CANVAS, "{precision:?}");
        }
    }
}
