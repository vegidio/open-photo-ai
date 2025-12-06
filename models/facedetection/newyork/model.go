package newyork

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const (
	targetSize          = 640
	numAnchors          = 16800
	confidenceThreshold = 0.5
)

type NewYork struct {
	id        string
	name      string
	operation OpFdNewYork
	session   *ort.DynamicAdvancedSession
}

func New(operation types.Operation) (*NewYork, error) {
	op := operation.(OpFdNewYork)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Face Detection (%s)",
		cases.Upper(language.English).String(string(op.precision)),
	)

	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	if err := utils.PrepareDependency(url, "models", modelFile, nil); err != nil {
		return nil, err
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"loc", "conf", "landmarks"},
	)
	if err != nil {
		return nil, err
	}

	return &NewYork{
		name:      name,
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[[]facedetection.Face] = (*NewYork)(nil)

// region - Model methods

func (m *NewYork) Id() string {
	return m.operation.Id()
}

func (m *NewYork) Name() string {
	return m.name
}

func (m *NewYork) Run(input *types.InputImage, onProgress types.ProgressCallback) ([]facedetection.Face, error) {
	if onProgress != nil {
		onProgress("fd", 0)
	}

	// Preprocess image
	inputData, originalWidth, originalHeight := facedetection.PreprocessImage(input.Pixels, targetSize)

	if onProgress != nil {
		onProgress("fd", 0.2)
	}

	// Create input tensor
	inputShape := ort.NewShape(1, 3, int64(targetSize), int64(targetSize))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, fmt.Errorf("failed to create input tensor: %w", err)
	}
	defer inputTensor.Destroy()

	// Create output tensors
	locTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 4))
	if err != nil {
		return nil, fmt.Errorf("failed to create loc tensor: %w", err)
	}
	defer locTensor.Destroy()

	confTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 2))
	if err != nil {
		return nil, fmt.Errorf("failed to create conf tensor: %w", err)
	}
	defer confTensor.Destroy()

	landmarksTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 10))
	if err != nil {
		return nil, fmt.Errorf("failed to create landmarks tensor: %w", err)
	}
	defer landmarksTensor.Destroy()

	// Run inference
	err = m.session.Run([]ort.Value{inputTensor}, []ort.Value{locTensor, confTensor, landmarksTensor})
	if err != nil {
		return nil, fmt.Errorf("failed to run inference: %w", err)
	}

	if onProgress != nil {
		onProgress("fd", 0.6)
	}

	// Post-process results
	locData := locTensor.GetData()
	confData := confTensor.GetData()
	landmarksData := landmarksTensor.GetData()

	faces := facedetection.PostProcessDetections(locData, confData, landmarksData,
		originalWidth, originalHeight, confidenceThreshold)

	if onProgress != nil {
		onProgress("fd", 1)
	}

	return faces, nil
}

func (m *NewYork) Destroy() {
	m.session.Destroy()
}

// endregion
