package petersburg

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// divergenceThreshold is the max |raw output| above which a tile is treated as a NAFNet blow-up and replaced with the
// original input pixels. 3.0 sits safely above legitimate output magnitude (~O(1)) and far below the ~1000+ blow-up.
const divergenceThreshold = 3.0

type Petersburg struct {
	name      string
	operation OpShPetersburg
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Petersburg, error) {
	op := operation.(OpShPetersburg)

	session, err := sharpen.LoadSession(ctx, "petersburg", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Petersburg session")
	}

	return &Petersburg{
		name:      sharpen.FormatSharpenName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Petersburg)(nil)

// region - Model methods

func (m *Petersburg) Id() string {
	return m.operation.Id()
}

func (m *Petersburg) Name() string {
	return m.name
}

func (m *Petersburg) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	result, err := sharpen.RunPipeline(ctx, m.session, img, onProgress, utils.WithDivergenceGuard(divergenceThreshold))
	if err != nil {
		return nil, err
	}

	// Amplify (or soften) the sharpening by extrapolating the residual at the per-run intensity; intensity 1.0 returns
	// the model output unchanged.
	return utils.BlendWithIntensity(img, result, utils.IntensityFromParams(params)), nil
}

func (m *Petersburg) Destroy() {
	m.session.Destroy()
}

// endregion
