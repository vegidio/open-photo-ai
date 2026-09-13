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

// The CoreML sweep, on an M2 Max, macOS 26.6, ONNX Runtime 1.26, per 256x256 tile.
//
// The fact that decides everything else: CoreML takes this graph as ONE partition and puts all of it on the GPU, in
// both precisions. That is the opposite of its two neighbours in this package, and the reason is architectural.
// Moscow and novgorod are Restormer, whose blocks are normalization and transpose around a channel-attention matmul;
// CoreML reaches for the Neural Engine there and loses. NAFNet is a plain convolutional U-Net - 226 convolutions,
// half the estimated graph cost - and CoreML declines the Neural Engine on its own, so ALL and CPUAndGPU are the same
// session.
//
//	MLComputeUnits           fp32              fp16
//	ALL (default)            34.4ms            34.3ms
//	CPUAndGPU                34.2ms  (-0.5%)   34.4ms  (+0.2%)
//	CPUAndNeuralEngine      248.1ms  (+622%)   37.0ms  (+7.7%)
//	CPUOnly / CPU provider  471.9ms           468.6ms
//
// So the setting that is worth 45% on moscow is worth nothing here, and setting it the way moscow does would only
// restate a choice CoreML has already made.
//
// FastPrediction, sequential and AllowLowPrecisionAccumulationOnGPU all landed inside the run-to-run spread, measured
// in both precisions with the order alternated per round. AllowLowPrecisionAccumulationOnGPU has no field on
// EPProfile and was reached through the provider options directly rather than by adding one - the order to keep,
// since a typed field for a setting no model uses is harder to delete than it was to add.
//
// One thing that is not a tuning result but changes what to recommend: the fp16 export is not faster. End to end on
// the 640x640 perftest sample it is 328ms against fp32's 328ms, because CoreML compiles both to the same GPU program.
// Unlike novgorod, where the precision choice is also a speed choice, picking fp16 here buys download size and
// nothing else.

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
