package uppsala

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Uppsala struct {
	name      string
	operation OpDnUppsala
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Uppsala, error) {
	op := operation.(OpDnUppsala)

	session, err := denoise.LoadSession(ctx, "uppsala", op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Uppsala session")
	}

	return &Uppsala{
		name:      denoise.FormatDenoiseName(op.precision),
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Uppsala)(nil)

// region - Model methods

func (m *Uppsala) Id() string {
	return m.operation.Id()
}

func (m *Uppsala) Name() string {
	return m.name
}

func (m *Uppsala) Run(
	ctx context.Context,
	img image.Image,
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return denoise.RunPipeline(ctx, m.session, img, onProgress)
}

func (m *Uppsala) Destroy() {
	m.session.Destroy()
}

// endregion
