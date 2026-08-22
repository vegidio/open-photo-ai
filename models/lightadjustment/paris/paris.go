package paris

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/lightadjustment"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to paris; the shared implementation lives in the lightadjustment package.
var variant = &lightadjustment.Variant{
	Codename: "paris",
	Label:    "Paris",
}

// New loads the paris session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*lightadjustment.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a paris operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) lightadjustment.Op {
	return variant.Op(intensity, precision)
}
