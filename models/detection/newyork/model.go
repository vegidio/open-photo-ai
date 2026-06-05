package newyork

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const confidenceThreshold = 0.5

type NewYork struct {
	name      string
	operation OpDtNewYork
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*NewYork, error) {
	op, ok := operation.(OpDtNewYork)
	if !ok {
		return nil, errors.Errorf("expected OpDtNewYork operation, got %T", operation)
	}

	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("New York (%s)", cases.Upper(language.English).String(string(op.precision)))
	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)

	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: op.Hash(),
	}

	if err := utils.PrepareDependency(ctx, url, "models", fileCheck, onProgress); err != nil {
		return nil, errors.Wrap(err, "failed to prepare New York model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"loc", "conf", "landmarks"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create New York session")
	}

	return &NewYork{
		name:      name,
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[[]detection.Face] = (*NewYork)(nil)

// region - Model methods

func (m *NewYork) Id() string {
	return m.operation.Id()
}

func (m *NewYork) Name() string {
	return m.name
}

func (m *NewYork) Run(
	ctx context.Context,
	img image.Image,
	_ map[string]any,
	onProgress types.InferenceProgress,
) ([]detection.Face, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}
	if onProgress != nil {
		onProgress("dt", 0)
	}

	// Preprocess image
	inputData, originalWidth, originalHeight := detection.PreprocessImage(img, detection.TargetSize)

	// Create input tensor
	inputShape := ort.NewShape(1, 3, int64(detection.TargetSize), int64(detection.TargetSize))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	// Create output tensors. The anchor count is derived from the model's fixed input size so it stays in sync with
	// the anchor grid in package detection.
	numAnchors := int64(detection.AnchorCount())
	locTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 4))
	if err != nil {
		return nil, errors.Wrap(err, "failed to create loc tensor")
	}
	defer locTensor.Destroy()

	confTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 2))
	if err != nil {
		return nil, errors.Wrap(err, "failed to create conf tensor")
	}
	defer confTensor.Destroy()

	landmarksTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 10))
	if err != nil {
		return nil, errors.Wrap(err, "failed to create landmarks tensor")
	}
	defer landmarksTensor.Destroy()

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}
	if onProgress != nil {
		onProgress("dt", 0.2)
	}

	// Run inference
	err = m.session.Run([]ort.Value{inputTensor}, []ort.Value{locTensor, confTensor, landmarksTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	// Post-process results
	locData := locTensor.GetData()
	confData := confTensor.GetData()
	landmarksData := landmarksTensor.GetData()

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}
	if onProgress != nil {
		onProgress("dt", 0.6)
	}

	faces := detection.PostProcessDetections(locData, confData, landmarksData,
		originalWidth, originalHeight, confidenceThreshold)

	if onProgress != nil {
		onProgress("dt", 1)
	}

	return faces, nil
}

func (m *NewYork) Destroy() {
	m.session.Destroy()
}

// endregion
