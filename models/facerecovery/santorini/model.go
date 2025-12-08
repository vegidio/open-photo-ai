package santorini

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const (
	tileSize = 512
)

type Santorini struct {
	id        string
	name      string
	operation OpFrSantorini
	session   *ort.DynamicAdvancedSession
	fdModel   types.Model[[]facedetection.Face]
}

func New(operation types.Operation) (*Santorini, error) {
	// Init the Face Detection model, which is a dependency of this model
	fdModel, err := facerecovery.GetFdModel()
	if err != nil {
		return nil, err
	}

	op := operation.(OpFrSantorini)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Face Recovery (%s)",
		cases.Upper(language.English).String(string(op.precision)),
	)

	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	if err = utils.PrepareDependency(url, "models", modelFile, nil); err != nil {
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

	return &Santorini{
		name:      name,
		operation: op,
		session:   session,
		fdModel:   fdModel,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.OutputImage] = (*Santorini)(nil)

// region - Model methods

func (m *Santorini) Id() string {
	return m.operation.Id()
}

func (m *Santorini) Name() string {
	return m.name
}

func (m *Santorini) Run(input *types.InputImage, onProgress types.ProgressCallback) (*types.OutputImage, error) {
	if onProgress != nil {
		onProgress("fr", 0)
	}

	faces, err := m.fdModel.Run(input, nil)
	if err != nil {
		return nil, err
	}

	if onProgress != nil {
		onProgress("fr", 0.1)
	}

	if len(faces) == 0 {
		return &types.OutputImage{
			FilePath: input.FilePath,
			Pixels:   input.Pixels,
			Format:   types.FormatJpeg,
		}, nil
	}

	result, err := facerecovery.RestoreFaces(m.session, input.Pixels, faces, tileSize, -1, onProgress)
	if err != nil {
		return nil, err
	}

	return &types.OutputImage{
		FilePath: input.FilePath,
		Pixels:   result,
		Format:   types.FormatJpeg,
	}, nil
}

func (m *Santorini) Destroy() {
	m.session.Destroy()

	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion
