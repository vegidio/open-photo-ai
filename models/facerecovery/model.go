package facerecovery

import (
	"fmt"
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type FaceRecovery struct {
	id        string
	name      string
	operation OpFaceRecovery
	session   *ort.DynamicAdvancedSession
	appName   string
}

const (
	modelTag = "face-recovery/1.0.0" // The place where the models are stored
	tileSize = 256                   // Fixed size for all tiles (static shape for ONNX)
	tilePad  = 10                    // Padding to avoid seam artifacts
)

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.OutputImage] = (*FaceRecovery)(nil)

func New(appName string, operation types.Operation) (*FaceRecovery, error) {
	op := operation.(OpFaceRecovery)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Face Recovery (%s, %s)",
		cases.Title(language.English).String(string(op.mode)),
		cases.Upper(language.English).String(string(op.precision)),
	)

	// Download the model, if needed
	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/%s", modelTag, modelFile)
	if err := utils.PrepareDependency(appName, url, "models", modelFile, nil); err != nil {
		return nil, err
	}

	session, err := utils.CreateSession(appName, modelFile, []string{"input"}, []string{"output"})
	if err != nil {
		return nil, err
	}

	return &FaceRecovery{
		name:      name,
		operation: op,
		session:   session,
		appName:   appName,
	}, nil
}

// region - Model methods

func (m *FaceRecovery) Id() string {
	return m.operation.Id()
}

func (m *FaceRecovery) Name() string {
	return m.name
}

func (m *FaceRecovery) Run(input *types.InputImage, onProgress func(float32)) (*types.OutputImage, error) {
	//width := input.Pixels.Bounds().Dx()
	//height := input.Pixels.Bounds().Dy()

	return nil, nil
}

func (m *FaceRecovery) Destroy() {
	m.session.Destroy()
}

// endregion

// region - Private methods

func (m *FaceRecovery) upscaleTile(tile image.Image) (*image.RGBA, error) {
	return nil, nil
}

// endregion
