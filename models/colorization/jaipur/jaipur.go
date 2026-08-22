package jaipur

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/colorization"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to jaipur; the shared implementation lives in the colorization package.
var variant = &colorization.Variant{
	Codename: "jaipur",
	Label:    "Jaipur",
	Spec:     colorization.DeOldify,
}

// New loads the jaipur session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorization.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a jaipur operation at the given precision.
func Op(precision types.Precision) colorization.Op {
	return variant.Op(precision)
}
