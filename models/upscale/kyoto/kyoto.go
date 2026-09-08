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
// # Why only fp16
//
// CoreML's typed execution bars an fp32 MLProgram from the Neural Engine altogether, so at fp32 there is nothing to
// choose: MLComputeUnits=ALL already means CPU and GPU. Asking for the Neural Engine anyway does not get it, it
// drops the graph onto the CPU - a 9x regression at 2x and a 20x one at 4x on an M2 Max. So the fp32 passes get no
// setting.
//
// # The fp16 passes
//
// This is an RRDBNet - 351 convolutions, 279 LeakyRelus and 276 concatenations, no attention anywhere - which is
// exactly the dense-convolution mix the Neural Engine is built for. At 2x, ALL already finds the Neural Engine and
// naming it changes nothing; at 4x, ALL splits the graph and loses 25% to the transitions (211.1ms against 168.9ms).
// Naming it is what makes the two passes behave the same way, which is the point: the setting is not there for the 2x
// tie, it is there so the 4x pass stops being scheduled differently from its sibling.
//
// # This depends on the export, and that is the larger half of the story
//
// The same comparison against the PREVIOUS export says the opposite - 4x fp16 measured 288.8ms on the Neural Engine
// against 231.8ms on the GPU, and CPUAndGPU was the setting this profile originally carried.
//
// What changed is not the tuning but the graph. That export ran its two upsample Resize nodes in fp32, because
// onnxconverter-common blocks Resize when it converts a model to float16, which left four Cast nodes wrapped around
// the largest tensors in the graph - the 64-channel feature maps at 512x512 and 1024x1024. Two fp32 islands in the
// middle of an fp16 graph are enough to keep CoreML from giving the whole thing to the Neural Engine, and the
// thrashing that follows is what the old numbers were measuring.
//
// Resize is on that block list for a typing reason rather than a numerical one - its scales parameter is fixed to
// tensor(float) by the ONNX spec, so a blanket conversion produces a model ORT rejects at load - and in nearest
// mode it only copies values, so fp16 is exact there. Unblocking it and repairing the parameter type is worth
// 354.5ms to 168.9ms end to end on the 4x fp16 pass, of which the profile is the smaller share.
//
// The lesson generalises past kyoto: on CoreML, measure an fp16 graph's compute units only after checking that the
// float16 conversion did not leave fp32 islands in it. Tuning around one measures the conversion, not the model.
//
// End to end on the 640x640 sample at fp16, the re-export plus this profile is worth 1.13x at 2x, 1.89x at 4x and
// 1.32x at 8x. 8x gains least because it is nine tiles through the 4x graph followed by 121 through the 2x one, and
// the 2x pass was already reaching the Neural Engine on its own.
//
// # What was measured and left alone
//
// SpecializationStrategy, AllowLowPrecisionAccumulationOnGPU and the execution mode are all within about 3% on this
// graph with bit-identical output, so the profile names none of them.
//
// The execution mode is worth a note because tokyo sets it and kyoto deliberately does not. Under CoreML the
// question cannot arise: the provider takes all 1024 nodes as a single partition, so the inter-op pool has one node
// to schedule whatever the mode says. On the CPU provider, where there really are 1024 nodes, sequential is a small
// consistent loss - RRDB's dense blocks feed each concatenation from several branches at once, which is the wide,
// independent structure the pool exists for and the one SwinIR's chain of attention blocks does not have.
//
// # Quality
//
// The Neural Engine result is no further from an fp32 CPU reference than what shipped before it: 1.86/255 maximum
// deviation at 68.8 dB, against the previous configuration's 1.55/255 at 68.9 dB. The GPU is the more accurate of
// the two at 1.09/255 and 73.7 dB, but 36% slower, and neither difference is visible.
//
// # Portability
//
// CPUAndNeuralEngine makes ORT throw at session build on a Mac with no Neural Engine, which would drop this model
// to the CPU provider. That cannot be reached here: no darwin_amd64 ONNX Runtime is published (see
// internal/artifacts.go), so every Mac that runs inference at all is Apple Silicon and has one. Publishing an Intel
// runtime would make this setting conditional.
//
// The 20% itself is this machine's number, and the balance between the Neural Engine and the GPU differs across
// Apple Silicon generations. The direction should hold - it follows from the op mix, which is the same everywhere -
// but re-measure before quoting the margin on other hardware.
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
