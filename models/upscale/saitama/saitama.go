package saitama

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to saitama; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Saitama",
	Codename:     "saitama",
	ScaleBuckets: upscale.DefaultScaleBuckets,
}

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
