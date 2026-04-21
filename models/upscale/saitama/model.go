package saitama

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

type Saitama struct {
	name      string
	operation OpUpSaitama
	sessions  []*ort.DynamicAdvancedSession
	scales    []int
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Saitama, error) {
	op := operation.(OpUpSaitama)
	scales := selectScaleMatrix(op.scale)

	sessions, err := upscale.LoadSessions(ctx, "saitama", op.precision, scales, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Saitama sessions")
	}

	return &Saitama{
		name:      upscale.FormatUpscaleName(op.scale, op.precision),
		operation: op,
		sessions:  sessions,
		scales:    scales,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Saitama)(nil)

// region - Model methods

func (m *Saitama) Id() string {
	return m.operation.Id()
}

func (m *Saitama) Name() string {
	return m.name
}

func (m *Saitama) Run(
	ctx context.Context,
	img image.Image,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return upscale.RunPipeline(ctx, m.sessions, img, m.scales, m.operation.scale, onProgress)
}

func (m *Saitama) Destroy() {
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
