package petersburg

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/sharpen"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

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
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return sharpen.RunPipeline(ctx, m.session, img, onProgress)
}

func (m *Petersburg) Destroy() {
	m.session.Destroy()
}

// endregion
