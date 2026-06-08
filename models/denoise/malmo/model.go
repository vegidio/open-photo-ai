package malmo

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Malmo struct {
	name      string
	operation OpDnMalmo
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Malmo, error) {
	op := operation.(OpDnMalmo)

	session, err := denoise.LoadSession(ctx, "malmo", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Malmo session")
	}

	return &Malmo{
		name:      denoise.FormatDenoiseName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Malmo)(nil)

// region - Model methods

func (m *Malmo) Id() string {
	return m.operation.Id()
}

func (m *Malmo) Name() string {
	return m.name
}

func (m *Malmo) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	result, err := denoise.RunPipeline(ctx, m.session, img, onProgress)
	if err != nil {
		return nil, err
	}

	// Amplify (or soften) the denoising by extrapolating the residual at the per-run intensity; intensity 1.0 returns
	// the model output unchanged.
	return utils.BlendWithIntensity(img, result, utils.IntensityFromParams(params)), nil
}

func (m *Malmo) Destroy() {
	m.session.Destroy()
}

// endregion
