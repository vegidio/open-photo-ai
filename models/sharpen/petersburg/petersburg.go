package petersburg

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to petersburg; the shared implementation lives in the sharpen package.
var variant = &sharpen.Variant{
	Codename: "petersburg",
	Label:    "Petersburg",
	// DivergenceThreshold is the max |raw output| above which a tile is treated as a NAFNet blow-up and
	// replaced with the original input pixels. 3.0 sits safely above legitimate output magnitude (~O(1)) and far
	// below the ~1000+ blow-up.
	DivergenceThreshold: 3.0,
}

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
