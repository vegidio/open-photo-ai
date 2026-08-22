package athens

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to athens; the shared implementation lives in the facerecovery package.
var variant = &facerecovery.Variant{
	Codename: "athens",
	Label:    "Athens",
	// CodeFormer takes the fidelity weight as a second input; 1.0 is full fidelity to the original face.
	Inputs:   []string{"input", "weight"},
	TileSize: 512,
	Fidelity: 1.0,
}

// New loads the athens session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*facerecovery.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds an athens operation at the given precision, for the given pre-detected faces.
func Op(precision types.Precision, faces []detection.Face) facerecovery.Op {
	return variant.Op(precision, faces)
}
