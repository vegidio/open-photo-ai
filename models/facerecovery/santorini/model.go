package santorini

import (
	"context"
	"fmt"

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

func New(operation types.Operation, onProgress types.DownloadProgress) (*Santorini, error) {
	fdModel, modelFile, err := facerecovery.LoadModel(operation, onProgress)
	if err != nil {
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

	modelName := fmt.Sprintf("Santorini (%s)", cases.Upper(language.English).String(string(operation.Precision())))

	return &Santorini{
		name:      modelName,
		operation: operation.(OpFrSantorini),
		session:   session,
		fdModel:   fdModel,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.ImageData] = (*Santorini)(nil)

// region - Model methods

func (m *Santorini) Id() string {
	return m.operation.Id()
}

func (m *Santorini) Name() string {
	return m.name
}

func (m *Santorini) Run(
	ctx context.Context,
	input *types.ImageData,
	params map[string]any,
	onProgress types.InferenceProgress,
) (*types.ImageData, error) {
	faces, err := facerecovery.ExtractFaces(ctx, m.fdModel, input, onProgress)
	if err != nil {
		return nil, err
	}

	if len(faces) == 0 {
		return &types.ImageData{
			FilePath: input.FilePath,
			Pixels:   input.Pixels,
		}, nil
	}

	result, err := facerecovery.RestoreFaces(ctx, m.session, input.Pixels, faces, tileSize, -1, onProgress)
	if err != nil {
		return nil, err
	}

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   result,
	}, nil
}

func (m *Santorini) Destroy() {
	m.session.Destroy()

	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion
