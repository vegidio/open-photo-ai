package opai

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// SuggestEnhancements analyzes the input image and returns a list of recommended enhancement.
//
// It evaluates the image for potential face recovery, light adjustment, and upscaling improvements based on image
// characteristics such as detected faces and resolution.
func SuggestEnhancements(input *types.ImageData) []types.ModelType {
	enhancementTypes := make([]types.ModelType, 0)

	if yes := shouldFaceRecovery(input); yes {
		enhancementTypes = append(enhancementTypes, types.ModelTypeFaceRecovery)
	}

	if yes := shouldLightAdjustment(input); yes {
		enhancementTypes = append(enhancementTypes, types.ModelTypeLightAdjustment)
	}

	if yes := shouldUpscale(input); yes {
		enhancementTypes = append(enhancementTypes, types.ModelTypeUpscale)
	}

	return enhancementTypes
}

// region - Private functions

func shouldFaceRecovery(input *types.ImageData) bool {
	model, err := facerecovery.GetFdModel(types.ExecutionProviderAuto)
	if err != nil {
		return false
	}

	faces, err := facerecovery.ExtractFaces(context.Background(), model, input.Pixels, nil)
	if err != nil {
		return false
	}

	return len(faces) > 0
}

func shouldLightAdjustment(input *types.ImageData) bool {
	bounds := input.Pixels.Bounds()
	totalPixels := float64(bounds.Dx() * bounds.Dy())

	if totalPixels == 0 {
		return false
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
		return true
	}

	// Too bright
	if meanLuminance > meanBrightLimit && brightRatio > clippingRatio {
		return true
	}

	return false
}

func shouldUpscale(input *types.ImageData) bool {
	const _2Mp = 4_194_304

	bounds := input.Pixels.Bounds()
	mp := bounds.Dx() * bounds.Dy()

	return mp <= _2Mp
}

// endregion
