package moscow

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to moscow; the shared implementation lives in the sharpen package.
var variant = &sharpen.Variant{
	Codename: "moscow",
}

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
