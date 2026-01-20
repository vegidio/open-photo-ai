package lightadjustment

import (
	"image"
	"image/color"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// BlendWithIntensity blends the original image with the model output based on intensity.
// intensity = 1.0: full model output
// intensity = 0.0: original image
// intensity = -1.0: opposite of model adjustment (inverse effect)
func BlendWithIntensity(original, modelOutput image.Image, intensity float32) image.Image {
	bounds := original.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	result := image.NewNRGBA(image.Rect(0, 0, width, height))

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			origR, origG, origB, origA := original.At(x, y).RGBA()
			modelR, modelG, modelB, _ := modelOutput.At(x, y).RGBA()

			// Convert to float32 in range [0, 255]
			oR := float32(origR) / 257.0
			oG := float32(origG) / 257.0
			oB := float32(origB) / 257.0

			mR := float32(modelR) / 257.0
			mG := float32(modelG) / 257.0
			mB := float32(modelB) / 257.0

			// Blend formula: result = original + intensity * (modelOutput - original)
			// This allows extrapolation beyond the range for negative intensity
			r := oR + intensity*(mR-oR)
			g := oG + intensity*(mG-oG)
			b := oB + intensity*(mB-oB)

			result.Set(x, y, color.NRGBA{
				R: uint8(utils.ClampFloat32(r)),
				G: uint8(utils.ClampFloat32(g)),
				B: uint8(utils.ClampFloat32(b)),
				A: uint8(origA / 257),
			})
		}
	}

	return result
}
