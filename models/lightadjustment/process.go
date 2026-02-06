package lightadjustment

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
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
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Create input tensor with dynamic shape
	inputShape := ort.NewShape(1, 3, int64(height), int64(width))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	// Create output tensor
	outputShape := ort.NewShape(1, 3, int64(height), int64(width))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create output tensor")
	}
	defer outputTensor.Destroy()

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Run inference
	if err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor}); err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Convert output tensor back to image
	outputData := outputTensor.GetData()
	outputImg := utils.CHWToImage(outputData, width, height, false)

	return outputImg, nil
}
