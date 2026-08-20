package stockholm

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to stockholm; the shared implementation lives in the denoise package.
var variant = &denoise.Variant{
	Codename: "stockholm",
	// DivergenceThreshold is the max |raw output| above which a tile is treated as a NAFNet blow-up and
	// replaced with the original input pixels. 3.0 sits safely above legitimate output magnitude (~O(1)) and far
	// below the ~1000+ blow-up.
	DivergenceThreshold: 3.0,
}

// New loads the stockholm session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*denoise.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a stockholm operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) denoise.Op {
	return variant.Op(intensity, precision)
}
