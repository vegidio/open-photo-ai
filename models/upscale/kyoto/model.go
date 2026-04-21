package kyoto

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Kyoto struct {
	name      string
	operation OpUpKyoto
	sessions  []*ort.DynamicAdvancedSession
	scales    []int
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Kyoto, error) {
	op := operation.(OpUpKyoto)
	scales := selectScaleMatrix(op.scale)

	sessions, err := upscale.LoadSessions(ctx, "kyoto", op.precision, scales, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Kyoto sessions")
	}

	return &Kyoto{
		name:      upscale.FormatUpscaleName(op.scale, op.precision),
		operation: op,
		sessions:  sessions,
		scales:    scales,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Kyoto)(nil)

// region - Model methods

func (m *Kyoto) Id() string {
	return m.operation.Id()
}

func (m *Kyoto) Name() string {
	return m.name
}

func (m *Kyoto) Run(
	ctx context.Context,
	img image.Image,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return upscale.RunPipeline(ctx, m.sessions, img, m.scales, m.operation.scale, onProgress)
}

func (m *Kyoto) Destroy() {
	for _, session := range m.sessions {
		session.Destroy()
	}
}

// endregion

// region - Private functions

func selectScaleMatrix(scale float64) []int {
	switch {
	case scale <= 2:
		return []int{2}
	case scale <= 4:
		return []int{4}
	case scale <= 8:
		return []int{4, 2}
	default:
		return []int{}
	}
}

// endregion
