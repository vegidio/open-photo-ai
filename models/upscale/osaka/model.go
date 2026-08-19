package osaka

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Osaka is the SeedVR2-backed diffusion upscaler: a VAE encoder, a one-step diffusion transformer and a VAE decoder,
// run as a single pass over the image at its target size.
type Osaka struct {
	name      string
	operation OpUpOsaka
	ep        types.ExecutionProvider
	utils.Sessions

	// Aliases into Sessions, for readability at the call sites. They are not counted twice: ResidentBytes and Destroy
	// are promoted from the embedded slice, which is the only owner.
	dit *utils.Session
	enc *utils.Session
	dec *utils.Session
}

func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Osaka, error) {
	op := operation.(OpUpOsaka)

	sessions, err := loadSessions(ctx, op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Osaka sessions")
	}

	return &Osaka{
		// The name deliberately omits the scale, unlike the other upscalers. One set of sessions serves every scale,
		// so a name naming one would be frozen at whichever scale happened to build the model first.
		name:      utils.FormatModelName("Upscale", op.precision),
		operation: op,
		ep:        ep,
		Sessions:  sessions,
		dit:       sessions[0],
		enc:       sessions[1],
		dec:       sessions[2],
	}, nil
}

var _ types.Model[image.Image] = (*Osaka)(nil)
var _ types.Measurable = (*Osaka)(nil)

// region - Model methods

func (m *Osaka) Id() string {
	return m.operation.Id()
}

func (m *Osaka) Name() string {
	return m.name
}

// Run restores the image at the scale carried in params. The scale is a per-run input rather than part of the model
// identity, so one set of sessions serves every scale the user picks.
func (m *Osaka) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return m.runPipeline(ctx, img, ScaleFromParams(params), onProgress)
}

// endregion
