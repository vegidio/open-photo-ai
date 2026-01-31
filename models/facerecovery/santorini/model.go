package santorini

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
	tileSize = 512
)

type Santorini struct {
	name      string
	operation OpFrSantorini
	session   *ort.DynamicAdvancedSession
	fdModel   types.Model[[]facedetection.Face]
}

func New(operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Santorini, error) {
	fdModel, modelFile, err := facerecovery.LoadModel(operation, ep, onProgress)
	if err != nil {
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

	modelName := fmt.Sprintf("Santorini (%s)", cases.Upper(language.English).String(string(operation.Precision())))

	return &Santorini{
		name:      modelName,
		operation: operation.(OpFrSantorini),
		session:   session,
		fdModel:   fdModel,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Santorini)(nil)

// region - Model methods

func (m *Santorini) Id() string {
	return m.operation.Id()
}

func (m *Santorini) Name() string {
	return m.name
}

func (m *Santorini) Run(
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

	result, err := facerecovery.RestoreFaces(ctx, m.session, img, faces, tileSize, -1, onProgress)
	if err != nil {
		return nil, err
	}

	return result, nil
}

func (m *Santorini) Destroy() {
	m.session.Destroy()

	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion
