package denoise

import (
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// runInference runs denoising inference on a single tile. The input and output share the same shape (no scaling); the
// tile is expected to already be padded to the model's fixed input size.
func runInference(session *ort.DynamicAdvancedSession, tile image.Image) (image.Image, error) {
	bounds := tile.Bounds()
	h, w := bounds.Dy(), bounds.Dx()

	inputData := utils.ImageToCHW(tile, true, false)

	shape := ort.NewShape(1, 3, int64(h), int64(w))
	inputTensor, err := ort.NewTensor(shape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	outputTensor, err := ort.NewEmptyTensor[float32](shape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create output tensor")
	}
	defer outputTensor.Destroy()

	err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	return utils.CHWToImage(outputTensor.GetData(), w, h, false), nil
}
