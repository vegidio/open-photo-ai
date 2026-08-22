package santorini

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to santorini; the shared implementation lives in the facerecovery package.
var variant = &facerecovery.Variant{
	Codename: "santorini",
	Label:    "Santorini",
	// The graph takes the image alone, so there is no fidelity weight to bind; -1 marks its absence.
	Inputs:   []string{"input"},
	TileSize: 512,
	Fidelity: -1,
}

// New loads the santorini session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*facerecovery.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a santorini operation at the given precision, for the given pre-detected faces.
func Op(precision types.Precision, faces []detection.Face) facerecovery.Op {
	return variant.Op(precision, faces)
}
