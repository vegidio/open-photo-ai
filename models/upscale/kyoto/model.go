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
	scales := upscale.SelectScaleMatrix(op.scale, kyotoScaleBuckets)

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
	_ map[string]any,
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

// kyotoScaleBuckets reflects Kyoto's native 2x and 4x models (8x = 4x then 2x).
var kyotoScaleBuckets = []upscale.ScaleBucket{
	{Max: 2, Passes: []int{2}},
	{Max: 4, Passes: []int{4}},
	{Max: 8, Passes: []int{4, 2}},
}

// endregion
