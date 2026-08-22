package mumbai

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/colorization"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to mumbai; the shared implementation lives in the colorization package.
var variant = &colorization.Variant{
	Codename: "mumbai",
	Label:    "Mumbai",
	Spec:     colorization.DDColor,
}

// New loads the mumbai session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorization.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a mumbai operation at the given precision.
func Op(precision types.Precision) colorization.Op {
	return variant.Op(precision)
}
