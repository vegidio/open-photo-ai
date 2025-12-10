package tokyo

import (
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

type Tokyo struct {
	id        string
	name      string
	operation OpUpTokyo
	session   *ort.DynamicAdvancedSession
}

func New(operation types.Operation) (*Tokyo, error) {
	op := operation.(OpUpTokyo)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Upscale %dx (%s)",
		op.scale,
		cases.Upper(language.English).String(string(op.precision)),
	)

	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	if err := utils.PrepareDependency(url, "models", modelFile, nil); err != nil {
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

	return &Tokyo{
		name:      name,
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.ImageData] = (*Tokyo)(nil)

// region - Model methods

func (m *Tokyo) Id() string {
	return m.operation.Id()
}

func (m *Tokyo) Name() string {
	return m.name
}

func (m *Tokyo) Run(input *types.ImageData, onProgress types.ProgressCallback) (*types.ImageData, error) {
	if onProgress != nil {
		onProgress("up", 0)
	}

	result, err := upscale.Process(m.session, input.Pixels, tileSize, tileOverlap, m.operation.scale, onProgress)
	if err != nil {
		return nil, err
	}

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   result,
	}, nil
}

func (m *Tokyo) Destroy() {
	m.session.Destroy()
}

// endregion
