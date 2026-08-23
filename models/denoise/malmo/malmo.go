package malmo

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to malmo; the shared implementation lives in the denoise package.
var variant = &denoise.Variant{
	Codename: "malmo",
	Label:    "Malmo",
}

// New loads the malmo session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*denoise.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a malmo operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) denoise.Op {
	return variant.Op(intensity, precision)
}
