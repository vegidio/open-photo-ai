package osaka

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// The roles the pipeline asks for its three graphs by. Binding by name is what keeps a reordering of the slice below
// from silently swapping the encoder and the decoder.
const (
	roleDiT     = "dit"
	roleEncoder = "encoder"
	roleDecoder = "decoder"
)

// variant holds everything specific to osaka; the shared implementation lives in the upscale package.
//
// Osaka is the SeedVR2-backed diffusion upscaler: a VAE encoder, a one-step diffusion transformer and a VAE decoder,
// run as a single pass over the image at its target size. Unlike the convolutional upscalers, which hold one session
// per scale factor, these three are stages of a single pass and are always loaded together - which is why it uses the
// diffusion contract rather than ScaleBuckets.
var variant = &upscale.Variant{
	Codename: "osaka",
	Label:    "Osaka",
	Diffusion: &upscale.DiffusionSpec{
		Graphs:    graphs,
		Profile:   profileFor,
		Precision: types.PrecisionFp16,
		Run:       runPipeline,
	},
}

// New loads the osaka sessions for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*upscale.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds an osaka operation at the given scale.
//
// The precision argument is accepted for symmetry with the other upscalers but ignored: only an fp16 build of this
// model is published, and the variant pins it.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}
