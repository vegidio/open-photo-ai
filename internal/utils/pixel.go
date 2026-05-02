package utils

import (
	"image"
	"image/color"
	"math"
)

// BlendWithIntensity blends the original image with the model output based on intensity.
//
//   - intensity = 1.0: full model output
//   - intensity = 0.0: original image
//   - intensity = -1.0: opposite of model adjustment (inverse effect)
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
				R: uint8(ClampFloat32(r)),
				G: uint8(ClampFloat32(g)),
				B: uint8(ClampFloat32(b)),
				A: uint8(origA / 257),
			})
		}
	}

	return result
}

// RgbToHsv converts an RGB color to HSV (Hue, Saturation, Value). All input channels (r, g, b) are expected to be in
// the range [0, 255].
//
// The returned values are:
//   - h: hue in degrees, in the range [0, 360)
//   - s: saturation, scaled to the range [0, 255]
//   - v: value (brightness), in the range [0, 255]
func RgbToHsv(r, g, b float64) (h, s, v float64) {
	maxC := math.Max(r, math.Max(g, b))
	minC := math.Min(r, math.Min(g, b))
	delta := maxC - minC

	v = maxC

	if maxC == 0 {
		s = 0
	} else {
		s = (delta / maxC) * 255.0
	}

	if delta == 0 {
		h = 0
	} else {
		switch maxC {
		case r:
			h = 60.0 * math.Mod((g-b)/delta, 6.0)
		case g:
			h = 60.0 * (((b - r) / delta) + 2.0)
		default:
			h = 60.0 * (((r - g) / delta) + 4.0)
		}
	}

	if h < 0 {
		h += 360.0
	}

	return
}
