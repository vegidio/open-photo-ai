package tokyo

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

type Tokyo struct {
	name      string
	operation OpUpTokyo
	sessions  []*utils.Session
	scales    []int
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Tokyo, error) {
	op := operation.(OpUpTokyo)
	scales := upscale.SelectScaleMatrix(op.scale, upscale.DefaultScaleBuckets)

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
var _ types.Measurable = (*Tokyo)(nil)

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
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return upscale.RunPipeline(ctx, m.sessions, img, m.scales, m.operation.scale, onProgress)
}

// ResidentBytes reports the size of the model files backing this session, which is what the registry
// budgets against when deciding what can stay loaded.
func (m *Tokyo) ResidentBytes() int64 {
	return utils.SessionsBytes(m.sessions)
}

func (m *Tokyo) Destroy() {
	for _, session := range m.sessions {
		session.Destroy()
	}
}

// endregion
