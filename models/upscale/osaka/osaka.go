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
		Graphs:  graphs,
		Profile: profileFor,
		Run:     runPipeline,
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

// Op builds an osaka operation at the given scale and precision.
//
// Two precisions are published: fp16, and int8 for the diffusion transformer alone - half the download and about
// twice as fast on the CPU, measured visually lossless against fp16. Both share one pair of fp16 VAE halves; see
// graphs in loader.go.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}
