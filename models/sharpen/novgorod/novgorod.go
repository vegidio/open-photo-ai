package novgorod

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to novgorod; the shared implementation lives in the sharpen package.
var variant = &sharpen.Variant{
	Codename: "novgorod",
}

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
