package athens

import (
	"context"
	"fmt"
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
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
	name      string
	operation OpFrAthens
	session   *ort.DynamicAdvancedSession
	fdModel   types.Model[[]facedetection.Face]
}

func New(operation types.Operation, onProgress types.DownloadProgress) (*Athens, error) {
	fdModel, modelFile, err := facerecovery.LoadModel(operation, onProgress)
	if err != nil {
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

	modelName := fmt.Sprintf("Athens (%s)", cases.Upper(language.English).String(string(operation.Precision())))

	return &Athens{
		name:      modelName,
		operation: operation.(OpFrAthens),
		session:   session,
		fdModel:   fdModel,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Athens)(nil)

// region - Model methods

func (m *Athens) Id() string {
	return m.operation.Id()
}

func (m *Athens) Name() string {
	return m.name
}

func (m *Athens) Run(
	ctx context.Context,
	img image.Image,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	faces, err := facerecovery.ExtractFaces(ctx, m.fdModel, img, onProgress)
	if err != nil {
		return nil, err
	}

	if len(faces) == 0 {
		return img, nil
	}

	result, err := facerecovery.RestoreFaces(ctx, m.session, img, faces, tileSize, fidelity, onProgress)
	if err != nil {
		return nil, err
	}

	return result, nil
}

func (m *Athens) Destroy() {
	m.session.Destroy()

	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion
