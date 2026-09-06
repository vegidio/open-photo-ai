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

	// The geometry paris has always run: a dynamic-shape graph capped at a 1024 longest side and reflection-padded
	// to a multiple of 16. These were constants in process.go until lyon needed different ones; the values are
	// unchanged, and TestPlanCanvasParisMatchesLegacyGeometry pins them against the original arithmetic.
	Canvas: lightadjustment.Canvas{MaxSize: 1024, Align: 16},

	// No Profile: nobody has measured paris. See the note on Variant.Profile - a nil Profile is how "this graph has
	// not been measured" is spelled, and it is not the same as "the defaults were measured and won".
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
