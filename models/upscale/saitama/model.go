package saitama

import (
	"context"
	"fmt"
	"image"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Saitama struct {
	name      string
	operation OpUpSaitama
	sessions  []*ort.DynamicAdvancedSession
	scales    []int
}

func New(operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Saitama, error) {
	op := operation.(OpUpSaitama)
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
			return nil, err
		}

		session, err := utils.CreateSession(
			modelFile,
			[]string{"input"},
			[]string{"output"},
			ep,
		)
		if err != nil {
			return nil, err
		}

		sessions = append(sessions, session)
	}

	return &Saitama{
		name:      name,
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
