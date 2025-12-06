package athens

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facedetection/newyork"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const (
	fidelity = 1.0
	tileSize = 512
)

type Athens struct {
	id        string
	name      string
	operation OpFrAthens
	session   *ort.DynamicAdvancedSession
	fdModel   types.Model[[]facedetection.Face]
}

func New(operation types.Operation) (*Athens, error) {
	// Init the Face Detection model, which is a dependency of this model
	fdModel, err := getFdModel()
	if err != nil {
		return nil, err
	}

	op := operation.(OpFrAthens)
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
		[]string{"input", "weight"},
		[]string{"output"},
	)
	if err != nil {
		return nil, err
	}

	return &Athens{
		name:      name,
		operation: op,
		session:   session,
		fdModel:   fdModel,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.OutputImage] = (*Athens)(nil)

// region - Model methods

func (m *Athens) Id() string {
	return m.operation.Id()
}

func (m *Athens) Name() string {
	return m.name
}

func (m *Athens) Run(input *types.InputImage, onProgress types.ProgressCallback) (*types.OutputImage, error) {
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

	result, err := facerecovery.RestoreFaces(m.session, input.Pixels, faces, tileSize, fidelity, onProgress)
	if err != nil {
		return nil, err
	}

	return &types.OutputImage{
		FilePath: input.FilePath,
		Pixels:   result,
		Format:   types.FormatJpeg,
	}, nil
}

func (m *Athens) Destroy() {
	m.session.Destroy()

	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion

func getFdModel() (types.Model[[]facedetection.Face], error) {
	var err error
	fdOp := newyork.Op(types.PrecisionFp32)

	model, exists := internal.Registry[fdOp.Id()]
	if !exists {
		model, err = newyork.New(fdOp)
		if err != nil {
			return nil, err
		}

		internal.Registry[fdOp.Id()] = model
	}

	return model.(types.Model[[]facedetection.Face]), nil
}
