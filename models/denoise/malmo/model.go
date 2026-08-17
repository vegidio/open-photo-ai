package malmo

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
)

type Malmo struct {
	name      string
	operation OpDnMalmo
	session   *utils.Session
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
var _ types.Measurable = (*Malmo)(nil)

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

// ResidentBytes reports the size of the model files backing this session, which is what the registry
// budgets against when deciding what can stay loaded.
func (m *Malmo) ResidentBytes() int64 {
	return m.session.Bytes()
}

func (m *Malmo) Destroy() {
	m.session.Destroy()
}

// endregion
