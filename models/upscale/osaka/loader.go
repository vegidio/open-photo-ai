package osaka

import (
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// The three graphs behind one Osaka model. Unlike the convolutional upscalers, which hold one session per scale
// factor, these are three stages of a single pass and are always loaded together.
//
// The tensor names are not shared with the rest of the codebase, which uses "input"/"output" everywhere: these graphs
// were exported with meaningful names.
//
// Only the diffusion transformer follows the operation's precision. It is where nearly all of the weight is - 6.8 GB
// against the VAE pair's 0.5 GB - so it alone was worth quantizing, and it is published both as fp16 and as int8.
// The two VAE halves are convolutional and published only as fp16: one pair of files, shared by both builds, which is
// why they pin their precision rather than following the operation into a `_int8` name that does not exist.
var graphs = []upscale.GraphSpec{
	{Role: roleDiT, Suffix: "", Inputs: []string{"vid_input", "timestep"}, Outputs: []string{"denoised_latent"}},

	{Role: roleEncoder, Suffix: "_vae_encoder", Precision: types.PrecisionFp16,
		Inputs: []string{"pixel_image"}, Outputs: []string{"latent"}},
	{Role: roleDecoder, Suffix: "_vae_decoder", Precision: types.PrecisionFp16,
		Inputs: []string{"latent"}, Outputs: []string{"pixel_image"}},
}

// profileFor is the provider tuning every Osaka graph needs.
//
// All three graphs have dynamic spatial axes, so ONNX Runtime's memory-pattern planner must be off: it assumes shapes
// repeat, and otherwise reserves for the largest region seen and never releases it. DynamicShapes says the same thing
// to any provider that would otherwise be told to expect fixed inputs.
//
// CoreML must not be used for this model. Measured through this code path against the ONNX Runtime the app bundles
// (1.26), across three separate attempts:
//
//   - the two VAE graphs fail session creation with "axis 4 is not in valid range [-4,3]". The VAE is a causal video
//     autoencoder whose 3D convolutions sit behind a 4-D boundary, and the CoreML EP mishandles the rank.
//   - the DiT creates a session and then aborts the process on the first Run, with the Metal assertion
//     "MPSNDArray initWithDevice: Error: device may not be nil".
//   - baking the rotary tables into constants does stop that abort, and CoreML is then 1.35x faster than the CPU -
//     but it returns the wrong answer: cosine 0.77 against the CPU result, worst element off by 6.1 on a +-7 range,
//     with no NaNs and a plausible range. That is the dangerous failure: a quietly degraded image and no error.
//
// The last point is why this exclusion matters more than a normal one. A provider that crashes announces itself; one
// that silently miscomputes does not. Do not re-enable on the strength of "it runs now" - compare the output against
// the CPU element by element, on both the DiT and the VAE.
//
// None of this is caused by the 960 re-export: the VAE files are unchanged from the original publication, so CoreML
// never worked for this pipeline.
//
// TensorRT needs explicit optimization profiles for dynamic inputs, and without them it either rebuilds an engine for
// every distinct tile size - minutes each - or grows an unbounded engine cache. Adding them means committing to shape
// ranges this pipeline does not yet have measurements for.
func profileFor() utils.EPProfile {
	return utils.EPProfile{
		DynamicShapes:     true,
		DisableMemPattern: true,
		DisableOptimizers: brokenOptimizers,
		ExcludeEPs: []types.ExecutionProvider{
			types.ExecutionProviderCoreML,
			types.ExecutionProviderTensorRT,
		},
	}
}

// brokenOptimizers are the ONNX Runtime graph transformers that miscompile this DiT.
//
// They rewrite the graph so it refers to a tensor they removed, and session creation then fails with a "name which
// does not exist" error naming a node they created - most recently
// "InsertedPrecisionFreeCast_/dit/vid_out_norm/Constant_output_0" for a SimplifiedLayerNormFusion node.
//
// This is a runtime bug, not an export one, and it is version-specific: the graph loads cleanly under ONNX Runtime
// 1.29, and fails under the 1.26 this app bundles. So it cannot be retired by re-exporting, only by moving the
// bundled runtime forward - and it must be checked against the bundled build rather than whatever a local Python
// install happens to have, which is how it was briefly and wrongly declared fixed.
var brokenOptimizers = []string{"ReshapeFusion", "SimplifiedLayerNormFusion"}
