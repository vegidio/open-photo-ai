package novgorod

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Novgorod struct {
	name      string
	operation OpShNovgorod
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Novgorod, error) {
	op := operation.(OpShNovgorod)

	session, err := sharpen.LoadSession(ctx, "novgorod", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Novgorod session")
	}

	return &Novgorod{
		name:      sharpen.FormatSharpenName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Novgorod)(nil)

// region - Model methods

func (m *Novgorod) Id() string {
	return m.operation.Id()
}

func (m *Novgorod) Name() string {
	return m.name
}

func (m *Novgorod) Run(
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

func (m *Novgorod) Destroy() {
	m.session.Destroy()
}

// endregion
