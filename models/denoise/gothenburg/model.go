package gothenburg

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Gothenburg struct {
	name      string
	operation OpDnGothenburg
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Gothenburg, error) {
	op := operation.(OpDnGothenburg)

	session, err := denoise.LoadSession(ctx, "gothenburg", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Gothenburg session")
	}

	return &Gothenburg{
		name:      denoise.FormatDenoiseName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Gothenburg)(nil)

// region - Model methods

func (m *Gothenburg) Id() string {
	return m.operation.Id()
}

func (m *Gothenburg) Name() string {
	return m.name
}

func (m *Gothenburg) Run(
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

func (m *Gothenburg) Destroy() {
	m.session.Destroy()
}

// endregion
