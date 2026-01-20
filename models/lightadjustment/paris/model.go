package paris

import (
	"context"
	"fmt"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/lightadjustment"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Paris struct {
	id        string
	name      string
	operation OpLaParis
	session   *ort.DynamicAdvancedSession
}

func New(operation types.Operation, onProgress types.DownloadProgress) (*Paris, error) {
	op := operation.(OpLaParis)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Paris (%s)", cases.Upper(language.English).String(string(op.precision)))
	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)

	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: op.Hash(),
	}

	if err := utils.PrepareDependency(url, "models", fileCheck, onProgress); err != nil {
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

	return &Paris{
		name:      name,
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.ImageData] = (*Paris)(nil)

// region - Model methods

func (m *Paris) Id() string {
	return m.operation.Id()
}

func (m *Paris) Name() string {
	return m.name
}

func (m *Paris) Run(
	ctx context.Context,
	input *types.ImageData,
	params map[string]any,
	onProgress types.InferenceProgress,
) (*types.ImageData, error) {
	if onProgress != nil {
		onProgress("la", 0)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	result, err := lightadjustment.Process(ctx, m.session, input.Pixels)
	if err != nil {
		return nil, err
	}

	intensity, exists := params["intensity"].(float32)
	if !exists {
		intensity = 0.5
	}

	if onProgress != nil {
		onProgress("la", 0.9)
	}
	if err = ctx.Err(); err != nil {
		return nil, err
	}

	blendedImg := lightadjustment.BlendWithIntensity(input.Pixels, result, intensity)

	if onProgress != nil {
		onProgress("la", 1)
	}

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   blendedImg,
	}, nil
}

func (m *Paris) Destroy() {
	m.session.Destroy()
}

// endregion
