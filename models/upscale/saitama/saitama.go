package saitama

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to saitama; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Saitama",
	Codename:     "saitama",
	ScaleBuckets: upscale.DefaultScaleBuckets,
	Profile:      profile,
}

// profile puts saitama's fp16 graph on the Neural Engine, and leaves fp32 alone.
//
// This is kyoto's profile arrived at independently, for the expected reason: both are RRDBNets, and this one is 96
// convolutions, 75 LeakyRelus and 72 concatenations with no attention anywhere - the dense-convolution mix the Neural
// Engine is built for.
//
// What makes it worth writing down separately is that the setting was worth NOTHING until the fp16 export was fixed,
// and against the export that shipped before it the Neural Engine was the worst of the three choices rather than the
// best. That export left its two upsample Resize nodes in fp32, because onnxconverter-common blocks Resize when it
// converts to float16 - the same defect kyoto had, from the same tool - putting four Cast nodes around the largest
// tensors in the graph. Partition counts do not reveal it: ORT hands CoreML all 282 nodes as one partition on both
// exports, and the split is inside the MLProgram, where CoreML has to move two fp32 islands off whichever engine is
// running the fp16 around them. Only a timing says it is wrong.
//
// The two exports invert the answer: on the published one the Neural Engine is 50% behind doing nothing, on the
// re-export it is the best by -54%, and the GPU is unchanged at 76ms on both - the GPU never cared about the fp32
// islands, the Neural Engine could not get past them. Tuning compute units against that export would have measured
// the conversion and shipped CPUAndGPU, which is a third of what is available here.
//
// ALL is not a substitute for naming the unit. It lands between the two, and its spread is the tell: 63.3ms fastest
// against a 72.9ms median, where every named configuration holds within 3ms of its own median. CoreML re-decides the
// placement, and the setting is what stops it.
//
// End to end on the 640x640 sample at 4x, fp16 goes 1.067s -> 677.7ms with the re-export -> 551.1ms with this
// profile. Quality is better than what shipped, not merely acceptable: 2.790/255 worst-pixel and 69.1 dB against an
// fp32 CPU-provider reference, where the published export on ALL was 3.973/255 and 68.7 dB.
//
// SpecializationStrategy, AllowLowPrecisionAccumulationOnGPU, EnableOnSubgraphs and the execution mode are all within
// 2% here with bit-identical output. The margins are this machine's; the direction follows from the op mix, which is
// the same everywhere, but re-measure before quoting the numbers on other hardware.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndNeuralEngine})

// New loads the saitama sessions for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*upscale.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a saitama operation at the given scale and precision.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}
