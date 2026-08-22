package newyork

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to newyork; the shared implementation lives in the detection package.
var variant = &detection.Variant{
	Codename: "newyork",
	Label:    "New York",
	Outputs:  []string{"loc", "conf", "landmarks"},
}

// New loads the newyork session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*detection.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a newyork operation at the given precision.
func Op(precision types.Precision) detection.Op {
	return variant.Op(precision)
}
