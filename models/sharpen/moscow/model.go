package moscow

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Moscow struct {
	name      string
	operation OpShMoscow
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Moscow, error) {
	op := operation.(OpShMoscow)

	session, err := sharpen.LoadSession(ctx, "moscow", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Moscow session")
	}

	return &Moscow{
		name:      sharpen.FormatSharpenName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Moscow)(nil)

// region - Model methods

func (m *Moscow) Id() string {
	return m.operation.Id()
}

func (m *Moscow) Name() string {
	return m.name
}

func (m *Moscow) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	result, err := sharpen.RunPipeline(ctx, m.session, img, onProgress)
	if err != nil {
		return nil, err
	}

	// Amplify (or soften) the sharpening by extrapolating the residual at the per-run intensity; intensity 1.0 returns
	// the model output unchanged.
	return utils.BlendWithIntensity(img, result, utils.IntensityFromParams(params)), nil
}

func (m *Moscow) Destroy() {
	m.session.Destroy()
}

// endregion
