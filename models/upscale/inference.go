package upscale

import (
	"fmt"
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// runInference runs inference on a tile for upscaling
func runInference(session *ort.DynamicAdvancedSession, tile image.Image, upscale int) (image.Image, error) {
	bounds := tile.Bounds()
	h, w := bounds.Dy(), bounds.Dx()

	inputData := utils.ImageToCHW(tile, true, false)

	inputShape := ort.NewShape(1, 3, int64(h), int64(w))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, fmt.Errorf("create input tensor: %w", err)
	}
	defer inputTensor.Destroy()

	outputShape := ort.NewShape(1, 3, int64(h*upscale), int64(w*upscale))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, fmt.Errorf("create output tensor: %w", err)
	}
	defer outputTensor.Destroy()

	err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor})
	if err != nil {
		return nil, fmt.Errorf("run inference: %w", err)
	}

	outputData := outputTensor.GetData()
	outH := int(outputShape[2])
	outW := int(outputShape[3])

	return utils.CHWToImage(outputData, outW, outH, false), nil
}
