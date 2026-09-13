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
// 11.5 GB of resident memory. So ALL is not merely a slower default here, it is the path that fails.
//
// On speed alone the setting would only be worth it for fp16 (1.67x on an M2 Max; fp32 is a wash).
//
// # Execution mode
//
// The sequential mode is the larger of the two settings, and tokyo is the worst case in the catalogue for
// ExecutionModeParallel: 2682 nodes arranged as 54 window-attention blocks in a chain, so the inter-op pool never has
// two independent nodes to hand out and all it can do is charge a thread handoff per node.
//
// Worth 1.37x on CUDA (-26.9% at fp32, -18.1% at fp16), a smaller win on the CPU provider, nothing on a Mac at fp32
// and -3.5% at fp16. Output is bit-identical in both modes, which makes this a setting rather than a trade. TensorRT
// is a tie - it fuses the graph into one node - but setting the mode unconditionally is still right, because a machine
// without TensorRT runs on CUDA.
//
// # Provider options, and why none are set
//
// Nothing in cudaOptions or tensorRTOptions moves this graph. EXHAUSTIVE ties HEURISTIC because there are only 36
// Convs among 2682 nodes to search, and TF32 is worth 4.7% at fp32 over the 216 Gemms and 108 MatMuls. TensorRT
// swallows all 2682 nodes into a single engine, so there is no partitioning left to tune and every builder setting
// lands on the same fused engine.
//
// Two are actively wrong. prefer_nhwc loses in BOTH precisions, fp16 included - athens sets it and athens is also a
// transformer-bearing model, but NHWC pays for itself by reaching cuDNN's fp16 tensor-core kernels and 36
// convolutions are not enough of this graph to repay the layout conversions threaded through 344 transposes.
// fuse_conv_bias=1 is worse than a loss: at fp32 it returns garbage (max|d| 2.9e+07) and at fp16 it fails in the cuDNN
// frontend with HEURISTIC_QUERY_FAILED, the same way it does on santorini.
//
// # The one setting that does move it, and why it is not set
//
// trt_fp16_enable is worth 2.66x on the fp32 export, and it is exactly the fp16 export: the two half-precision rows
// agree on speed to 1.7% and on deviation from the fp32 reference to two significant figures. It does not produce a
// faster fp32 model, it produces the fp16 model under the fp32 model's name - and on TensorRT alone, so the same
// operation would carry a different precision depending on which provider the machine resolved to. It is the TensorRT
// twin of the ModelFormat=NeuralNetwork trap coreMLOptions describes, and declined for the same reason. The honest
// way to take the 2.66x is to select the fp16 model, which is what EPProfile.Fp16 is opt-in for.
//
// trt_bf16_enable is the same trade taken badly: -55.2% on the fp32 export at five times fp16's deviation, and +23.6%
// on the fp16 export, where it is a plain loss.
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
