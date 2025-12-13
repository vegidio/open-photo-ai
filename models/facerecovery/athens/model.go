package athens

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
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
	fdModel, modelFile, modelName, err := facerecovery.LoadModel(operation)
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

	return &Athens{
		name:      modelName,
		operation: operation.(OpFrAthens),
		session:   session,
		fdModel:   fdModel,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.ImageData] = (*Athens)(nil)

// region - Model methods

func (m *Athens) Id() string {
	return m.operation.Id()
}

func (m *Athens) Name() string {
	return m.name
}

func (m *Athens) Run(ctx context.Context, input *types.ImageData, onProgress types.ProgressCallback) (*types.ImageData, error) {
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

	result, err := facerecovery.RestoreFaces(ctx, m.session, input.Pixels, faces, tileSize, fidelity, onProgress)
	if err != nil {
		return nil, err
	}

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   result,
	}, nil
}

func (m *Athens) Destroy() {
	m.session.Destroy()

	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion
