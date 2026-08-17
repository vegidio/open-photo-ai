package saitama

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

type Saitama struct {
	name      string
	operation OpUpSaitama
	utils.Sessions
	scales []int
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Saitama, error) {
	op := operation.(OpUpSaitama)
	scales := upscale.SelectScaleMatrix(op.scale, upscale.DefaultScaleBuckets)

	sessions, err := upscale.LoadSessions(ctx, "saitama", op.precision, scales, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Saitama sessions")
	}

	return &Saitama{
		name:      upscale.FormatUpscaleName(op.scale, op.precision),
		operation: op,
		Sessions:  sessions,
		scales:    scales,
	}, nil
}

var _ types.Model[image.Image] = (*Saitama)(nil)
var _ types.Measurable = (*Saitama)(nil)

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
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	return upscale.RunPipeline(ctx, m.Sessions, img, m.scales, m.operation.scale, onProgress)
}

// endregion
