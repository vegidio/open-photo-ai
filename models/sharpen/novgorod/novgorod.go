package novgorod

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to novgorod; the shared implementation lives in the sharpen package.
var variant = &sharpen.Variant{
	Codename: "novgorod",
	Label:    "Novgorod",
	Profile:  profile,
}

// profile keeps the fp16 export off the Neural Engine.
//
// Restormer is a transformer, not the convolutional stack the Neural Engine is built for: its 44 blocks are mostly
// layer normalization, reshape and transpose around a channel-attention matmul. Left to ALL, CoreML takes the Neural
// Engine anyway and spends more time crossing on and off it than it saves - 180 ms per 256x256 tile against 143 ms on
// CPUAndGPU, measured on an M2 Max. That difference is what decides the precision: at 143 ms the fp16 export is the
// fastest way to run this model, and at 180 ms it is slower than fp32's 152 ms, so a user picking fp16 for speed would
// have got the opposite.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// New loads the novgorod session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*sharpen.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a novgorod operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) sharpen.Op {
	return variant.Op(intensity, precision)
}
