//! Jaipur: the DeOldify model, and the family's only graph that returns a colorized photograph rather than its chroma.
//!
//! [`rgb`] is the contract it runs, at model tier because Jaipur is the only model that has it. The rest of the run is
//! the family's [`process`](super::process).

// Deleting the model is deleting this directory plus the arms in `ColorizationVariant` that name it.
//
// A second RGB-output model would promote `rgb` to the family tier, beside `ab`.

pub(in crate::models::colorization) mod rgb;

use crate::models::precision::Precision;
use crate::providers::profile::{EpProfile, cpu_and_gpu_at_fp16};

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, the provider defaults at FP32, and two nodes kept off WebGPU at both.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `jaipur.go`, not re-measured on this project's build.
    //
    // Kept off the Neural Engine at FP16, which on this graph is worth roughly 4.5x:
    //
    //   Compute units            FP16
    //   Default                  955ms
    //   CPU and GPU              209ms
    //   CPU and Neural Engine    235ms
    //
    // The CPU-and-Neural-Engine figure being four times FASTER than the default is what identifies the cost. It is not
    // the Neural Engine executing the graph: it is CoreML's planner splitting the graph across the GPU and the Neural
    // Engine and paying a transition at every crossing. Pinning either unit alone avoids it, and the default is the only
    // setting that cannot.
    //
    // Worth recording because Jaipur contradicts the op-mix reasoning: a DeOldify U-Net over a ResNet34 body is as
    // convolutional as they come — 57 Convs and 55 Relus out of 197 nodes — and it still wants the default off. So op
    // mix alone is not a reason to leave a graph on the default. It is the same answer as Delhi's and Mumbai's, but
    // measured on this graph rather than inherited from them: those are DDColor, a different architecture, and neither
    // family's figures are evidence for the other's.
    //
    // Sequential execution and CoreML's fast-prediction hint were both swept here and sit inside the noise floor,
    // bit-identical, and neither is set.
    //
    // Export note: Jaipur is DeOldify's ARTISTIC generator — a ResNet34 body with DynamicUnetDeep at nf_factor 1.5 — not
    // a newer training run of the ResNet101/DynamicUnetWide graph that shipped before it, so anything measured against
    // that one is not comparable. fastai's PixelShuffle_ICNR blurs with a ReplicationPad2d, and MLProgram supports only
    // `constant` and `reflect` padding, so those five Pads have to be rewritten as a Slice+Concat of the border row and
    // column BEFORE tracing to get one CoreML partition. It is the same trap as Mumbai's.
    //
    // The WebGPU pin is a correctness fix rather than tuning. The two 3x3 convolutions at the top of the decoder — 303
    // channels in and out, at the full canvas — are the one place in the published graphs where the WebGPU plugin hangs
    // the GPU outright on Mesa's RADV driver: the second consuming the first's output loses the device, and the run
    // fails with *"[Device] is lost"* (microsoft/onnxruntime#32777). 303 is not a multiple of four, which probably
    // sends the plugin to its unvectorised convolution; the same two layers at 304 channels are fine, as is either one
    // alone. They are about half the graph's multiply-adds, so Jaipur gains far less from the GPU than the other models
    // do, but it gains rather than failing.
    EpProfile { webgpu_cpu_nodes: WEBGPU_CPU_NODES.to_vec(), ..cpu_and_gpu_at_fp16(precision) }
}

/// The decoder's two 303-channel convolutions that hang the GPU on the WebGPU provider.
const WEBGPU_CPU_NODES: [&str; 2] = ["/m/layers.10/layers.0/layers.0.0/Conv", "/m/layers.10/layers.1/layers.1.0/Conv"];

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::colorization::ColorizationVariant;
    use crate::models::precision::FloatPrecision;

    #[test]
    fn jaipur_declares_the_shared_fp16_profile_and_its_webgpu_pins_through_its_variant() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                ColorizationVariant::Jaipur(precision).profile(),
                EpProfile { webgpu_cpu_nodes: WEBGPU_CPU_NODES.to_vec(), ..cpu_and_gpu_at_fp16(precision.into()) },
                "{precision:?}"
            );
        }
    }
}
