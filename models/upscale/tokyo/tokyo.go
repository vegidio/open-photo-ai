package tokyo

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to tokyo; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Tokyo",
	Codename:     "tokyo",
	ScaleBuckets: upscale.DefaultScaleBuckets,
	Profile:      profileFor,
}

// profileFor keeps tokyo off the Neural Engine and off the inter-op thread pool.
//
// # CoreML compute units
//
// CPUAndGPU is chosen for what it excludes as much as for what it measures: it is the one setting that cannot reach
// the ANE compiler, which cannot compile this graph at all. SwinIR is window attention - 452 reshapes, 344 transposes
// and 110 normalizations against 36 convolutions, the opposite of what a unit built for dense convolution wants.
// Asking for it produces `_ANECompiler : ANECCompile() FAILED`, one per transformer block, and the failures are not
// free: the CPUAndNeuralEngine session took 1067 seconds to build before running 22x slower than the GPU.
// SpecializationStrategy=FastPrediction, which also routes through that compiler, wedged the process outright at
// 11.5 GB of resident memory. ALL is not safe here merely because it is usually the right default - it lets CoreML
// try the Neural Engine, which is exactly the path that fails.
//
// On speed alone the setting would only be worth it for fp16 (1.67x on an M2 Max; fp32 is a wash). The cause is the
// graph's op mix and an ANE compiler limitation, both the same everywhere, so the direction should hold on any Apple
// Silicon Mac; on Intel there is no Neural Engine and the setting is a no-op.
//
// # Execution mode
//
// The sequential mode is the larger of the two settings here, and it belongs to no single provider. tokyo is the
// worst case in the catalogue for ExecutionModeParallel: 2682 nodes arranged as 54 window-attention blocks in a
// chain, so the inter-op pool never has two independent nodes to hand out and all it can do is charge a thread
// handoff per node.
//
// It is worth 1.37x on CUDA (-26.9% at fp32, -18.1% at fp16, graph alone and end to end alike), a smaller win on the
// CPU provider, nothing on a Mac at fp32 and -3.5% at fp16. Output is bit-identical between the two modes in both
// precisions on both platforms, which is what makes this a setting rather than a trade. TensorRT is the exception
// that proves the rule - it swallows the graph as one fused node and leaves the inter-op pool nothing to schedule -
// but setting the mode unconditionally is still right, because the CUDA provider is what a machine without TensorRT
// runs on.
//
// Whoever re-measures this: the Mac numbers are ratios, not absolutes. This graph is slow enough that thermal state
// moves it more than the mode does - the same fp16 parallel configuration measured 1619ms cold, 4780ms hot and
// 2319ms after a three-minute pause. Do not try to reproduce it end to end through perftest with one mode per
// process; that put one configuration at 12.2s and then 31.4s two rounds later. Interleave the modes in one process
// so drift moves both rows.
//
// # What the CUDA provider options cannot do for it
//
// Nothing in cudaOptions moves this graph, so the profile leaves them alone. EXHAUSTIVE and TF32, which is what
// cudaOptions already asks for, are the right answers - though only just: EXHAUSTIVE ties HEURISTIC because there are
// only 36 Convs among the 2682 nodes to search, and TF32 pays 4.7% at fp32 over the 216 Gemms and 108 MatMuls and
// nothing at fp16, where it does not apply.
//
// prefer_nhwc is the one to be careful about, because athens sets it and athens is also a transformer-bearing model.
// It loses here in both precisions, fp16 included, and the reason is the ratio rather than the precision: NHWC pays
// for itself by reaching cuDNN's fp16 tensor-core kernels, and 36 convolutions are not enough of this graph to repay
// the layout conversions threaded through 344 transposes. fuse_conv_bias=1 is worse than a loss - at fp32 it returns
// garbage (max|d| 2.9e+07 against the same input) and at fp16 it fails outright in the cuDNN frontend with
// HEURISTIC_QUERY_FAILED, the same way it does on santorini.
//
// Session-level settings land in the noise too, and the parallel mode's cost is not the CUDA stream fan-out it looks
// like: use_ep_level_unified_stream=1 *without* the sequential mode leaves the parallel baseline unchanged. It is the
// scheduling, not the streams.
//
// # What the TensorRT provider options cannot do for it either
//
// TrtOptions is empty for the same reason CudaOptions is, and the answer here is even flatter: TensorRT swallows all
// 2682 nodes into a single engine - the cached file ends `_0_0`, one subgraph - so there is no partitioning left to
// tune and every remaining option is a builder setting that lands on the same fused engine. Workspace size, auxiliary
// streams, builder optimization level, tactic sources, sparsity, context memory sharing and layer-norm fp32 fallback
// all measure as noise.
//
// Two traps are worth writing down for whoever sweeps the next graph, because both silently produce a clean table of
// wrong numbers. TensorRT's cache file name carries only the graph hash and the precision flags, so configurations
// sharing one engine cache directory hand each other the first one's engine and every option reads as a no-op. And
// the builder is non-deterministic: six sessions built from the SAME configuration spread from -2.5% to +1.2% at
// fp16, with five of six producing output differing from the first by 6.3e-3. One build per configuration cannot
// resolve anything below about +/-3%, so it is a sample of the builder rather than a measurement of the option.
//
// # The one setting that does move it, and why it is not set
//
// trt_fp16_enable is worth 2.66x on the fp32 export, and it is exactly the fp16 export: the two half-precision rows
// agree on speed to 1.7% and on deviation from the fp32 reference to two significant figures. Turning the flag on for
// the fp32 export does not produce a faster fp32 model, it produces the fp16 model under the fp32 model's name - and
// on TensorRT alone, since CUDA, CoreML and the CPU would keep running the graph as exported, so the same operation
// would carry a different precision depending on which provider the machine resolved to.
//
// So this is the TensorRT twin of the CoreML ModelFormat=NeuralNetwork trap described in coreMLOptions, and it is
// declined for the same reason: it is the largest number a provider sweep will find, and shipping it would hand a
// user who asked for fp32 a precision downgrade they never chose. The honest way to take the 2.66x is to select the
// fp16 model, which is what EPProfile.Fp16 is opt-in for.
//
// trt_bf16_enable is the same trade taken badly and needs no such care: -55.2% on the fp32 export at max|d| 3.3e-2 -
// slower than fp16 and five times the deviation - and +23.6% on the fp16 export, where it is a plain loss.
func profileFor(types.Precision) utils.EPProfile {
	return utils.EPProfile{
		CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU,
		ExecutionMode:      utils.ExecutionModeSequential,
	}
}

// New loads the tokyo sessions for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*upscale.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a tokyo operation at the given scale and precision.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}
