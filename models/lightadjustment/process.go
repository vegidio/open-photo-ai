package lightadjustment

import (
	"context"
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

func Process(ctx context.Context, session *ort.DynamicAdvancedSession, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	// Convert image CHW
	inputData := utils.ImageToCHW(img, false, false)

	if err := ctx.Err(); err != nil {
		return nil, err
	}

	// Create input tensor with dynamic shape
	inputShape := ort.NewShape(1, 3, int64(height), int64(width))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, err
	}
	defer inputTensor.Destroy()

	// Create output tensor
	outputShape := ort.NewShape(1, 3, int64(height), int64(width))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, err
	}
	defer outputTensor.Destroy()

	if err = ctx.Err(); err != nil {
		return nil, err
	}

	// Run inference
	if err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor}); err != nil {
		return nil, err
	}

	if err = ctx.Err(); err != nil {
		return nil, err
	}

	// Convert output tensor back to image
	outputData := outputTensor.GetData()
	outputImg := utils.CHWToImage(outputData, width, height, false)

	return outputImg, nil
}
