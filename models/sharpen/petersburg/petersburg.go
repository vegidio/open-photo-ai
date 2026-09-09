package petersburg

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to petersburg; the shared implementation lives in the sharpen package.
//
// Profile is nil, and here that means "measured, and the defaults won" rather than the "nobody has looked at this
// graph" the field's own documentation describes. The sweep is written down below so the next person does not pay
// for it again.
var variant = &sharpen.Variant{
	Codename: "petersburg",
	Label:    "Petersburg",
	// DivergenceThreshold is the max |raw output| above which a tile is treated as a NAFNet blow-up and
	// replaced with the original input pixels. 3.0 sits safely above legitimate output magnitude (~O(1)) and far
	// below the ~1000+ blow-up.
	DivergenceThreshold: 3.0,
}

// The CoreML sweep, on an M2 Max, macOS 26.6, ONNX Runtime 1.26. Times are per 256x256 tile unless stated.
//
// Start with the fact that decides everything else: CoreML takes this graph as ONE partition and puts all of it on
// the GPU, in both precisions. That is the opposite of what its two neighbours in this package do, and the reason is
// architectural rather than incidental. Moscow and novgorod are Restormer, whose blocks are normalization and
// transpose around a channel-attention matmul; CoreML reaches for the Neural Engine there and loses. NAFNet is a
// plain convolutional U-Net - 226 convolutions, half the estimated graph cost - and CoreML declines the Neural
// Engine on its own, so ALL and CPUAndGPU are the same session.
//
//	MLComputeUnits           fp32              fp16
//	ALL (default)            34.4ms            34.3ms
//	CPUAndGPU                34.2ms  (-0.5%)   34.4ms  (+0.2%)
//	CPUAndNeuralEngine      248.1ms  (+622%)   37.0ms  (+7.7%)
//	CPUOnly / CPU provider  471.9ms           468.6ms
//
// So the setting that is worth 45% on moscow is worth nothing here, and setting it the way moscow does would only
// restate a choice CoreML has already made. The fp32 row is the familiar trap rather than a finding: an MLProgram
// at fp32 cannot reach the Neural Engine at all, so CPUAndNeuralEngine there is a 7x fall back to the CPU.
//
// Nothing else moved either. Every number below is inside the run-to-run spread of the sweep that produced it, and
// each was measured in both precisions with the order alternated per round:
//
//	SpecializationStrategy FastPrediction     -1.0% fp32   +0.3% fp16
//	ExecutionMode sequential                  +0.9% fp32   +1.0% fp16
//	AllowLowPrecisionAccumulationOnGPU=1      -1.0% fp32    0.0% fp16
//
// ExecutionMode is the one worth understanding rather than just recording, because it is the largest win on other
// graphs in this codebase - tokyo takes -27% from it. It does nothing here for the same reason the compute units do
// nothing: one fused CoreML node leaves the inter-op thread pool with nothing to schedule. Node count is what
// predicts that setting, and this graph's 1244 nodes all collapse into one.
//
// AllowLowPrecisionAccumulationOnGPU has no field on EPProfile and is not reachable from here. It was measured
// through the provider options directly, before adding one - which is the order to keep, since a typed field for a
// setting no model uses is harder to delete than it was to add.
//
// One last thing that is not a tuning result but changes what to recommend: the fp16 export is not faster. End to
// end on the 640x640 perftest sample it is 328ms against fp32's 328ms, because CoreML compiles both to the same GPU
// program. Unlike novgorod, where the precision choice is also a speed choice, picking fp16 here buys download size
// and nothing else.

// New loads the petersburg session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*sharpen.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a petersburg operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) sharpen.Op {
	return variant.Op(intensity, precision)
}
