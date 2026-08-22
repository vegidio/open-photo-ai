package rio

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/colorbalance"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to rio; the shared implementation lives in the colorbalance package.
var variant = &colorbalance.Variant{
	Codename: "rio",
	Label:    "Rio",
}

// New loads the rio session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorbalance.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a rio operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) colorbalance.Op {
	return variant.Op(intensity, precision)
}
