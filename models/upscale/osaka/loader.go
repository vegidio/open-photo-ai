package osaka

import (
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// The three graphs behind one Osaka model. Unlike the convolutional upscalers, which hold one session per scale
// factor, these are three stages of a single pass and are always loaded together.
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
	{Role: roleDiT, Suffix: "", Profile: ditProfile,
		Inputs: []string{"vid_input"}, Outputs: []string{"denoised_latent"}},

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
// planner enabled, on an M2 Max. On the CUDA provider it is a tie on its own, but it turns bimodal once the execution
// mode below is sequential: half the passes land on 381ms and half on 498-508ms, with individual runs as bad as 963ms
// on the decoder. A setting that is free on average and occasionally costs 2.5x is not free.
//
// ExecutionMode is sequential, and on the CUDA provider it is the largest setting in this profile - larger than
// anything the provider itself offers. The DiT is 12,940 nodes, nearly five times tokyo's 2,682, and the inter-op
// thread pool is charged a handoff at every one of them while a graph this linear has no branch wide enough to use
// it. It is worth -13.5% at fp16 and -10.1% at int8 on the region, on all three graphs rather than one, with output
// identical to the parallel run. Being a session setting it reaches every provider, and on TensorRT it is a tie,
// because that provider fuses each graph into one node and leaves the inter-op pool nothing to schedule.
//
// End to end, the two settings together are worth -15.4% at fp16 and -11.4% at int8 through the CUDA provider. The
// gap between that and the -17.1% on the region is the pipeline around the model: the Lanczos resample, the wavelet
// colour fix and the canvas blend are CPU work that no provider setting touches.
//
// CudaPreferNHWC is set here for the two VAE halves and taken back off for the DiT; see ditProfile below for that
// half of it. The halves are 27 and 38 convolutions in fp16, which is the shape cuDNN has tensor-core kernels for in
// NHWC and which otherwise pays for a transpose on either side of every convolution. On top of sequential it is
// worth -8.0% on the encoder and -9.4% on the decoder, taking the region to -17.1% against the profile before either
// setting. The output is unchanged at cosine 1.00000 on all three graphs.
//
// There is no CudaOptions overlay, and that is a result rather than an omission: the CUDA provider's own options were
// swept and every one of them is already at its best value in the shared defaults.
//
//   - cudnn_conv_algo_search stays EXHAUSTIVE. DEFAULT costs +50.2%; HEURISTIC is an exact tie, and buys nothing on a
//     fixed-shape graph that searches once and then keeps the answer for the life of the session.
//   - use_tf32 stays on, and it matters more than everything else here put together. Turning it off takes the
//     decoder from 132.9ms to 23.565s and the encoder from 81.5ms to 933.6ms - the VAE's convolutions lose the
//     tensor-core path entirely and fall back to true fp32 accumulation.
//   - sdpa_kernel does nothing at either setting. ONNX Runtime only reaches those kernels through its fused Attention
//     and MultiHeadAttention contrib ops, and this DiT's attention is exported as 400 loose Softmax nodes that
//     nothing in the 1.26 fusion pipeline puts back together. A re-export that fused them is the only way this option
//     becomes reachable.
//   - arena_extend_strategy, do_copy_in_default_stream, cudnn_conv_use_max_workspace and
//     session.use_device_allocator_for_initializers are all ties inside the ~1% noise floor. The last two are worth
//     naming because they are the ones a reader reaches for on a 7 GB model, expecting the size to make them matter.
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
// TensorRT is no longer excluded either. The exclusion was inherited from the dynamic-shape export, where TensorRT
// had to rebuild an engine per distinct tile size; every graph is fixed-shape now, so there is one engine each and
// the objection is gone. It is the fastest provider this model has by a wide margin - 140.3ms a region against the
// CUDA provider's 441.1ms on an RTX 5090. It takes the DiT as a single subgraph, all 12,940 nodes of it, which is
// why trt_min_subgraph_size and trt_context_memory_sharing_enable do nothing here: there are no partition boundaries
// to tune.
//
// Fp16 is deliberately NOT set, and it is worth saying why for each export, because they fail the same test for
// opposite reasons. The fp16 graph gains nothing - TensorRT already honours the fp16 typing baked into the export.
// The int8 graph is fp32-typed apart from its quantized weights, so there the flag is not a no-op at all: it takes
// the DiT from 220.3ms to 78.4ms. It is still refused, because that speed is bought with precision the caller did
// not ask for - the DiT drifts to cosine 0.9827 against the same graph on CUDA, and the decoded region to 0.9954
// with a max absolute error of 2.0 on an image in [-1,1], where every other configuration here stays at 0.9999. The
// honest way to take that speed is to select the fp16 model, which is faster still and stays at cosine 0.9999.
//
// trt_int8_enable is refused for a second reason as well, and it is the one to remember when adding any option here:
// a profile applies to all three graphs. The quantization is weight-only DequantizeLinear with no Q on the
// activations, which TensorRT does not accelerate - the DiT measures as noise with it - but the flag reaches the two
// VAE halves too, and there it is a catastrophe: the encoder goes 21.4ms to 124.6ms and the decoder 45.5ms to
// 298.9ms. An option that helps the graph it was chosen for can wreck the other two.
//
// Measured and rejected as ties: a 24 GB workspace against the default 4 GB, trt_auxiliary_streams at 1 and at 4, and
// trt_layer_norm_fp32_fallback - which TensorRT itself suggests in a build warning, since the DiT exports its 255
// layer norms as ReduceMean/Pow/Sqrt/Div rather than the opset-17 LayerNormalization it would rather see. Taking that
// suggestion costs 1.3% and moves the result further from CUDA rather than nearer, so it stays off; a re-export using
// LayerNormalization would let TensorRT use INormalizationLayer and is the better way to answer that warning.
func profileFor(types.Precision) utils.EPProfile {
	return utils.EPProfile{
		DisableMemPattern:  true,
		DisableOptimizers:  brokenOptimizers,
		ExecutionMode:      utils.ExecutionModeSequential,
		CudaPreferNHWC:     true,
		CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU,
		TrtOptions:         trtOptions,
	}
}

// ditProfile is the variant's profile with prefer_nhwc taken back off for the diffusion transformer alone.
//
// The DiT contains no convolution whatsoever - 12,940 nodes of MatMul, Transpose and Softmax and not one Conv - so
// there is nothing for an NHWC convolution kernel to be faster at, and what is left is the layout transform's own
// cost. It is small but it is not noise: at fp16 the DiT is 201.6ms with NHWC off and 206.3ms with it on, with every
// individual pass on the same side of the line. On the int8 export the same comparison is noise - so this override
// neither helps nor hurts there, and it is set unconditionally rather than for fp16 alone so that the two builds keep
// the same provider configuration.
//
// This is the only per-graph override in the codebase, and it is worth being clear that it buys 1.3% of the region.
// The reason to spend an override on that rather than round it off is that the other side of the trade is not small -
// the same flag is worth -8.0% on the VAE encoder and -9.4% on the decoder - so the choice is between two real
// numbers rather than between a real one and a rounding error.
func ditProfile(precision types.Precision) utils.EPProfile {
	p := profileFor(precision)
	p.CudaPreferNHWC = false

	return p
}

// trtOptions is the one TensorRT setting this model wants that the shared defaults do not already give it.
//
// The builder optimization level is a build-time knob here and nothing else. Runtime is flat across levels 1, 3 and 5
// - 140.9ms, 140.3ms and 142.6ms, which is one spread of noise - while the engine build is 86s, 116s and 203s. So
// the 5 every other model gets buys this graph nothing and costs a minute and a half of the first run a user ever
// makes on a 7 GB model.
//
// It stops at 3, TensorRT's own default, rather than going to the 1 that measured the same, because level 0 is a
// cliff rather than a gentle slope: it builds in 82s and then runs the region in 533.7ms, worse than CUDA, with the
// VAE decoder alone going from 45.1ms to 296.4ms. Level 1 was fine on this card, but it is one step from that edge
// and the sweep behind it is a single GPU.
var trtOptions = map[string]string{
	"trt_builder_optimization_level": "3",
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
