package kyoto

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to kyoto; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Kyoto",
	Codename:     "kyoto",
	ScaleBuckets: kyotoScaleBuckets,
	Profile:      profile,
}

// profile puts kyoto's fp16 graphs on the Neural Engine, and leaves fp32 alone.
//
// This is an RRDBNet - 351 convolutions, 279 LeakyRelus and 276 concatenations, no attention anywhere - which is
// exactly the dense-convolution mix the Neural Engine is built for. At 2x, ALL already finds it and naming it changes
// nothing; at 4x, ALL splits the graph and loses 25% to the transitions (211.1ms against 168.9ms). Naming it is what
// makes the two passes behave the same way: the setting is not there for the 2x tie, it is there so the 4x pass stops
// being scheduled differently from its sibling.
//
// # This depends on the export, and that is the larger half of the story
//
// The same comparison against the PREVIOUS export says the opposite - 4x fp16 measured 288.8ms on the Neural Engine
// against 231.8ms on the GPU, and CPUAndGPU was the setting this profile originally carried. What changed is the
// graph, not the tuning. That export ran its two upsample Resize nodes in fp32, because onnxconverter-common blocks
// Resize when it converts a model to float16, leaving four Cast nodes wrapped around the largest tensors in the graph.
// Two fp32 islands in the middle of an fp16 graph are enough to keep CoreML from giving the whole thing to the Neural
// Engine, and the thrashing that follows is what the old numbers were measuring.
//
// Resize is on that block list for a typing reason rather than a numerical one - its scales parameter is fixed to
// tensor(float) by the ONNX spec - and in nearest mode it only copies values, so fp16 is exact there. Unblocking it
// and repairing the parameter type is worth 354.5ms to 168.9ms end to end on the 4x fp16 pass.
//
// The lesson generalises past kyoto: on CoreML, measure an fp16 graph's compute units only after checking that the
// float16 conversion did not leave fp32 islands in it. Tuning around one measures the conversion, not the model.
//
// SpecializationStrategy, AllowLowPrecisionAccumulationOnGPU and the execution mode are all within about 3% here with
// bit-identical output. Accuracy is unchanged from what shipped before (1.86/255 at 68.8 dB against 1.55/255 at
// 68.9 dB); the GPU is more accurate and 36% slower, and neither difference is visible.
//
// CPUAndNeuralEngine makes ORT throw at session build on a Mac with no Neural Engine, which cannot be reached here:
// no darwin_amd64 ONNX Runtime is published (see internal/artifacts.go), so every Mac that runs inference at all is
// Apple Silicon. Publishing an Intel runtime would make this setting conditional - saitama carries the same
// dependency. The margin is this machine's; re-measure before quoting it on other hardware.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndNeuralEngine})

// New loads the kyoto sessions for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*upscale.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a kyoto operation at the given scale and precision.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}

// kyotoScaleBuckets reflects Kyoto's native 2x and 4x models (8x = 4x then 2x).
var kyotoScaleBuckets = []upscale.ScaleBucket{
	{Max: 2, Passes: []int{2}},
	{Max: 4, Passes: []int{4}},
	{Max: 8, Passes: []int{4, 2}},
}
