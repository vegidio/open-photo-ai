package stockholm

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Stockholm struct {
	name      string
	operation OpDnStockholm
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Stockholm, error) {
	op := operation.(OpDnStockholm)

	session, err := denoise.LoadSession(ctx, "stockholm", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Stockholm session")
	}

	return &Stockholm{
		name:      denoise.FormatDenoiseName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Stockholm)(nil)

// region - Model methods

func (m *Stockholm) Id() string {
	return m.operation.Id()
}

func (m *Stockholm) Name() string {
	return m.name
}

func (m *Stockholm) Run(
	ctx context.Context,
	img image.Image,
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return denoise.RunPipeline(ctx, m.session, img, onProgress)
}

func (m *Stockholm) Destroy() {
	m.session.Destroy()
}

// endregion
