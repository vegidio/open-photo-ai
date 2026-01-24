package paris

import (
	"context"
	"fmt"
	"image"
	"regexp"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/lightadjustment"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Paris struct {
	name      string
	operation OpLaParis
	session   *ort.DynamicAdvancedSession
	intensity float32
}

func New(operation types.Operation, onProgress types.DownloadProgress) (*Paris, error) {
	op := operation.(OpLaParis)
	id := regexp.MustCompile(`_-?(?:0(?:\.\d+)?|1(?:\.0+)?)`).ReplaceAllString(op.Id(), "")
	modelFile := id + ".onnx"
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
var _ types.Model[image.Image] = (*Paris)(nil)

// region - Model methods

func (m *Paris) Id() string {
	return m.operation.Id()
}

func (m *Paris) Name() string {
	return m.name
}

func (m *Paris) Run(
	ctx context.Context,
	img image.Image,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress("la", 0)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	result, err := lightadjustment.Process(ctx, m.session, img)
	if err != nil {
		return nil, err
	}

	if onProgress != nil {
		onProgress("la", 0.9)
	}
	if err = ctx.Err(); err != nil {
		return nil, err
	}

	blendedImg := lightadjustment.BlendWithIntensity(img, result, m.operation.intensity)

	if onProgress != nil {
		onProgress("la", 1)
	}

	return blendedImg, nil
}

func (m *Paris) Destroy() {
	m.session.Destroy()
}

// endregion
