package detection

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

const confidenceThreshold = 0.5

// Run detects faces in img with a RetinaFace-style session: the graph takes a letterboxed CHW image at TargetSize and
// returns three tensors - box offsets, class scores and landmarks - one row per anchor, which are decoded back into
// image coordinates here.
func Run(
	ctx context.Context,
	session *utils.Session,
	img image.Image,
	onProgress types.InferenceProgress,
) ([]Face, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}
	if onProgress != nil {
		onProgress(0)
	}

	inputData, originalWidth, originalHeight := PreprocessImage(img, TargetSize)

	inputShape := ort.NewShape(1, 3, int64(TargetSize), int64(TargetSize))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	// Create output tensors. The anchor count is derived from the model's fixed input size so it stays in sync with
	// the anchor grid in this package.
	numAnchors := int64(AnchorCount())
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
		onProgress(0.2)
	}

	err = session.Run([]ort.Value{inputTensor}, []ort.Value{locTensor, confTensor, landmarksTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	locData := locTensor.GetData()
	confData := confTensor.GetData()
	landmarksData := landmarksTensor.GetData()

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}
	if onProgress != nil {
		onProgress(0.6)
	}

	faces := PostProcessDetections(locData, confData, landmarksData,
		originalWidth, originalHeight, confidenceThreshold)

	if onProgress != nil {
		onProgress(1)
	}

	return faces, nil
}
