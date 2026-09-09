package moscow

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to moscow; the shared implementation lives in the sharpen package.
var variant = &sharpen.Variant{
	Codename: "moscow",
	Label:    "Moscow",
	Profile:  profile,
}

// profile keeps the fp16 export off the Neural Engine, for the reason novgorod does: the two are the same Restormer
// backbone, and the op mix that decides this is a property of the architecture rather than of either checkpoint.
//
// The 44 blocks are mostly layer normalization, reshape and transpose around a channel-attention matmul, which is not
// what the Neural Engine is built for, and CoreML takes it anyway whenever ALL permits. End to end on the 640x640
// perftest sample, an M-series Mac: 5.067s on ALL against 2.803s on CPUAndGPU. Taking the GPU away instead of the
// Neural Engine (CPUAndNeuralEngine) is the direct confirmation of which half costs - it measures +59% per tile.
//
// The size of the win depends on the export, so re-measure it rather than trusting the figures above after a
// re-export. On a single-partition build of this graph the same switch is worth -22% per tile rather than -45%; the
// sign is the same, and the Neural Engine is the wrong place for this model either way.
//
// fp32 is left on the defaults by Fp16Only rather than by omission. An MLProgram at fp32 cannot reach the Neural
// Engine at all, so CPUAndGPU there only restates the default - measured at +0.4%, which is noise.
//
// Two settings measured as no-ops on this graph and are absent rather than untried: SpecializationStrategy
// FastPrediction (-0.4% fp32, +0.3% fp16) and ExecutionMode sequential (-0.1% fp32, 0.0% fp16). Both land inside the
// run-to-run spread, so neither earns a line here.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// New loads the moscow session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*sharpen.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a moscow operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) sharpen.Op {
	return variant.Op(intensity, precision)
}
