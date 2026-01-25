package opai

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/types"
)

// SuggestEnhancements analyzes the input image and returns a list of recommended enhancement operations.
//
// It evaluates the image for potential face recovery and upscaling improvements based on image characteristics such as
// detected faces and resolution.
func SuggestEnhancements(input *types.ImageData) ([]types.Operation, error) {
	operations := make([]types.Operation, 0)

	frOp, err := analyseFaceRecovery(input)
	if err != nil {
		return nil, err
	}
	operations = append(operations, frOp...)

	frLa, err := analyseLightAdjustment(input)
	if err != nil {
		return nil, err
	}
	operations = append(operations, frLa...)

	upOp, err := analyseUpscale(input)
	if err != nil {
		return nil, err
	}
	operations = append(operations, upOp...)

	return operations, nil
}

// region - Private functions

func analyseFaceRecovery(input *types.ImageData) ([]types.Operation, error) {
	operation := make([]types.Operation, 0)

	model, err := facerecovery.GetFdModel()
	if err != nil {
		return nil, err
	}

	faces, err := facerecovery.ExtractFaces(context.Background(), model, input.Pixels, nil)
	if err != nil {
		return nil, err
	}

	if len(faces) > 0 {
		operation = append(operation, athens.Op(types.PrecisionFp32))
	}

	return operation, nil
}

func analyseLightAdjustment(input *types.ImageData) ([]types.Operation, error) {
	operation := make([]types.Operation, 0)
	bounds := input.Pixels.Bounds()
	totalPixels := float64(bounds.Dx() * bounds.Dy())

	if totalPixels == 0 {
		return operation, nil
	}

	var sumLuminance float64
	var darkPixels int
	var brightPixels int

	// Thresholds (8-bit luminance)
	const (
		darkThreshold   = 15  // near black
		brightThreshold = 240 // near white
		meanDarkLimit   = 50
		meanBrightLimit = 200
		clippingRatio   = 0.35 // 35% pixels clipped
	)

	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			r, g, b, _ := input.Pixels.At(x, y).RGBA()

			// Convert from 16-bit to 8-bit
			r8 := float64(r >> 8)
			g8 := float64(g >> 8)
			b8 := float64(b >> 8)

			// Perceived luminance (Rec. 709)
			luminance := 0.2126*r8 + 0.7152*g8 + 0.0722*b8
			sumLuminance += luminance

			if luminance <= darkThreshold {
				darkPixels++
			} else if luminance >= brightThreshold {
				brightPixels++
			}
		}
	}

	meanLuminance := sumLuminance / totalPixels
	darkRatio := float64(darkPixels) / totalPixels
	brightRatio := float64(brightPixels) / totalPixels

	// Too dark
	if meanLuminance < meanDarkLimit && darkRatio > clippingRatio {
		operation = append(operation, paris.Op(0.5, types.PrecisionFp32))
	}

	// Too bright
	if meanLuminance > meanBrightLimit && brightRatio > clippingRatio {
		operation = append(operation, paris.Op(0.5, types.PrecisionFp32))
	}

	return operation, nil
}

func analyseUpscale(input *types.ImageData) ([]types.Operation, error) {
	const _1Mp = 1_048_576
	const _2Mp = 4_194_304

	operation := make([]types.Operation, 0)
	bounds := input.Pixels.Bounds()
	mp := bounds.Dx() * bounds.Dy()

	switch {
	case mp <= _1Mp:
		operation = append(operation, kyoto.Op(4, types.PrecisionFp32))
	case mp <= _2Mp:
		operation = append(operation, kyoto.Op(2, types.PrecisionFp32))
	}

	return operation, nil
}

// endregion
