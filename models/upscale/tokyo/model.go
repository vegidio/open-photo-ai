package tokyo

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Tokyo struct {
	name      string
	operation OpUpTokyo
	sessions  []*ort.DynamicAdvancedSession
	scales    []int
}

func New(operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Tokyo, error) {
	op := operation.(OpUpTokyo)
	name := fmt.Sprintf("Upscale %.4gx (%s)",
		op.scale,
		cases.Upper(language.English).String(string(op.precision)),
	)

	sessions := make([]*ort.DynamicAdvancedSession, 0)
	scales := selectScaleMatrix(op.scale)

	for _, scale := range scales {
		clonedOp := op
		clonedOp.scale = float64(scale)

		modelFile := clonedOp.Id() + ".onnx"
		url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
		fileCheck := &types.FileCheck{
			Path: modelFile,
			Hash: clonedOp.Hash(),
		}

		if err := utils.PrepareDependency(url, "models", fileCheck, onProgress); err != nil {
			return nil, errors.Wrap(err, "failed to prepare Tokyo model")
		}

		session, err := utils.CreateSession(
			modelFile,
			[]string{"input"},
			[]string{"output"},
			ep,
		)
		if err != nil {
			return nil, errors.Wrap(err, "failed to create Tokyo session")
		}

		sessions = append(sessions, session)
	}

	return &Tokyo{
		name:      name,
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
