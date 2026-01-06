package kyoto

import (
	"context"
	"fmt"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const (
	tileOverlap = 16
	tileSize    = 256
)

type Kyoto struct {
	id        string
	name      string
	operation OpUpKyoto
	sessions  []*ort.DynamicAdvancedSession
	scales    []int
}

func New(operation types.Operation, onProgress types.DownloadProgress) (*Kyoto, error) {
	op := operation.(OpUpKyoto)
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
		if err := utils.PrepareDependency(url, "models", modelFile, "", onProgress); err != nil {
			return nil, err
		}

		session, err := utils.CreateSession(
			modelFile,
			[]string{"input"},
			[]string{"output"},
		)
		if err != nil {
			return nil, err
		}

		sessions = append(sessions, session)
	}

	return &Kyoto{
		name:      name,
		operation: op,
		sessions:  sessions,
		scales:    scales,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.ImageData] = (*Kyoto)(nil)

// region - Model methods

func (m *Kyoto) Id() string {
	return m.operation.Id()
}

func (m *Kyoto) Name() string {
	return m.name
}

func (m *Kyoto) Run(ctx context.Context, input *types.ImageData, onProgress types.InferenceProgress) (*types.ImageData, error) {
	if onProgress != nil {
		onProgress("up", 0)
	}

	img := input.Pixels

	for i, session := range m.sessions {
		result, err := upscale.Process(ctx, session, img, tileSize, tileOverlap, m.scales[i], onProgress)
		if err != nil {
			return nil, err
		}

		img = result
	}

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   upscale.ResizeToIntendedScale(img, input.Pixels.Bounds(), m.operation.scale),
	}, nil
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
