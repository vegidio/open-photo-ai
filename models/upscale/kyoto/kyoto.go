package kyoto

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to kyoto; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Kyoto",
	Codename:     "kyoto",
	ScaleBuckets: kyotoScaleBuckets,
}

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
