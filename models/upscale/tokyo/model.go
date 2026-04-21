package tokyo

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Tokyo struct {
	name      string
	operation OpUpTokyo
	sessions  []*ort.DynamicAdvancedSession
	scales    []int
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Tokyo, error) {
	op := operation.(OpUpTokyo)
	scales := selectScaleMatrix(op.scale)

	sessions, err := upscale.LoadSessions(ctx, "tokyo", op.precision, scales, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Tokyo sessions")
	}

	return &Tokyo{
		name:      upscale.FormatUpscaleName(op.scale, op.precision),
		operation: op,
		sessions:  sessions,
		scales:    scales,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Tokyo)(nil)

// region - Model methods

func (m *Tokyo) Id() string {
	return m.operation.Id()
}

func (m *Tokyo) Name() string {
	return m.name
}

func (m *Tokyo) Run(
	ctx context.Context,
	img image.Image,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return upscale.RunPipeline(ctx, m.sessions, img, m.scales, m.operation.scale, onProgress)
}

func (m *Tokyo) Destroy() {
	for _, session := range m.sessions {
		session.Destroy()
	}
}

// endregion

// region - Private functions

func selectScaleMatrix(scale float64) []int {
	switch {
	case scale <= 4:
		return []int{4}
	case scale <= 8:
		return []int{4, 4}
	default:
		return []int{}
	}
}

// endregion
