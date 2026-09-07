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
// against the VAE pair's 0.17 GB - so it alone was worth quantizing, and it is published both as fp16 and as int8.
// The two VAE halves are convolutional and published only as fp16: one pair of files, shared by both builds, which is
// why they pin their precision rather than following the operation into a `_int8` name that does not exist.
//
// The DiT takes the packed latent alone. The timestep is baked into the graph as a constant, because this is a
// one-step model and the pipeline has only ever passed 1000 - see schedulerStep in pipeline.go, whose arithmetic
// collapses for exactly that reason. Leaving it as an input cost a rank-1 tensor per region and, more to the point,
// held the whole timestep embedding out of constant folding: 128 of the graph's 129 Sin/Cos pairs fold away once it
// is fixed, and what is left of that subgraph is what kept CoreML from taking the model in one piece.
var graphs = []upscale.GraphSpec{
	{Role: roleDiT, Suffix: "", Inputs: []string{"vid_input"}, Outputs: []string{"denoised_latent"}},

	{Role: roleEncoder, Suffix: "_vae_encoder", Precision: types.PrecisionFp16,
		Inputs: []string{"pixel_image"}, Outputs: []string{"latent"}},
	{Role: roleDecoder, Suffix: "_vae_decoder", Precision: types.PrecisionFp16,
		Inputs: []string{"latent"}, Outputs: []string{"pixel_image"}},
}

// profileFor is the provider tuning every Osaka graph needs.
//
// Every graph is fixed-shape: the DiT accepts one region size and nothing else, and the re-exported VAE halves are
// frozen at the 960x960 / 120x120 geometry restoreRegion is the only caller of. So DynamicShapes is deliberately not
// set, and that is load-bearing rather than tidy-up. It feeds CoreML's RequireStaticInputShapes, and setting it was
// what made the CoreML EP decline the VAE's every convolution and then fail session creation outright with
// "axis 4 is not in valid range [-4,3]" - a crash this profile was itself causing while the comment here blamed the
// runtime for it.
//
// DisableMemPattern stays on, but not for the reason it used to give. The shapes never vary, so the planner's
// assumption holds fine; it is simply a loss on activations this large - measured +22% on the VAE encoder with the
// planner enabled, on an M2 Max.
//
// CoreMLComputeUnits is the single largest provider knob here. The default ALL lets CoreML dispatch to the Neural
// Engine, which these graphs are consistently worse on: measured against CPUAndGPU on an M2 Max, ALL costs 4.2x on
// the VAE encoder and 3.9x on the decoder, and pinning the Neural Engine costs 2.5x on the DiT. SpecializationStrategy
// is left at its default - FastPrediction lands within noise on all three graphs and costs ~134 s of session build on
// the DiT.
//
// CoreML is no longer excluded. The three failures recorded here before - the VAE's rank error, the DiT's
// "MPSNDArray initWithDevice: Error: device may not be nil" abort, and a silently wrong result at cosine 0.77 - were
// all export defects, and all three are gone with the re-exported graphs: each is a single CoreML partition, and the
// output matches the CPU at cosine 0.99999 or better. The int8 DiT is the exception and stays fragmented at ~36
// partitions, because ONNX Runtime's CoreML EP has no DequantizeLinear builder in any form - per-channel or
// per-tensor, weight-only or full QDQ, opset 17 or 21 - so its 216 dequantize nodes cut the graph wherever they sit.
// It still runs correctly there, just slower than fp16; see Op in osaka.go for which build to prefer.
//
// The caution the old comment ended on still stands, though its specific findings no longer do: a provider that
// crashes announces itself, one that silently miscomputes does not. Anything that changes these graphs should be
// re-checked against the CPU element by element, on the DiT and both VAE halves.
//
// TensorRT needs explicit optimization profiles for dynamic inputs, and without them it either rebuilds an engine for
// every distinct tile size - minutes each - or grows an unbounded engine cache. The graphs are fixed-shape now, so
// that objection has largely dissolved, but nobody has measured this model on TensorRT since - hence the exclusion
// stays until someone does.
func profileFor(types.Precision) utils.EPProfile {
	return utils.EPProfile{
		DisableMemPattern:  true,
		DisableOptimizers:  brokenOptimizers,
		CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU,
		ExcludeEPs: []types.ExecutionProvider{
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
