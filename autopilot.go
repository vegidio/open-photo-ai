package opai

import (
	"context"
	"math"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// SuggestEnhancements analyzes the input image and returns a list of recommended enhancement.
//
// It evaluates the image for potential face recovery, light adjustment, color balance, and upscaling improvements based
// on image characteristics such as detected faces and resolution. The face-detection path may trigger a model download
// on first use; pass a cancellable ctx to abort it.
func SuggestEnhancements(ctx context.Context, input *types.ImageData) []types.ModelType {
	enhancementTypes := make([]types.ModelType, 0)

	if shouldFaceRecovery(ctx, input) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeFaceRecovery)
	}

	if shouldLightAdjustment(input) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeLightAdjustment)
	}

	if shouldColorBalance(input) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeColorBalance)
	}

	if shouldUpscale(input) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeUpscale)
	}

	return enhancementTypes
}

// region - Private functions

func shouldFaceRecovery(ctx context.Context, input *types.ImageData) bool {
	model, err := facerecovery.GetFdModel(ctx, types.ExecutionProviderAuto)
	if err != nil {
		return false
	}

	faces, err := facerecovery.ExtractFaces(ctx, model, input.Pixels, nil)
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

func shouldColorBalance(input *types.ImageData) bool {
	bounds := input.Pixels.Bounds()
	totalPixels := float64(bounds.Dx() * bounds.Dy())

	if totalPixels == 0 {
		return false
	}

	const (
		neutralSatCutoff        = 40.0
		neutralPixelMinRatio    = 0.02
		neutralCastThreshold    = 12.0
		hueBinCount             = 12
		neutralHueSkewThreshold = 0.45
		whiteBalanceThreshold   = 0.5
	)

	var sumR, sumG, sumB float64
	var neutralSumR, neutralSumG, neutralSumB float64
	neutralHueHistogram := make([]int, hueBinCount)
	var neutralPixels int

	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			r, g, b, _ := input.Pixels.At(x, y).RGBA()

			r8 := float64(r >> 8)
			g8 := float64(g >> 8)
			b8 := float64(b >> 8)

			sumR += r8
			sumG += g8
			sumB += b8

			h, s, _ := utils.RgbToHsv(r8, g8, b8)

			if s < neutralSatCutoff {
				neutralSumR += r8
				neutralSumG += g8
				neutralSumB += b8

				bin := int(h / 360.0 * float64(hueBinCount))
				if bin >= hueBinCount {
					bin = hueBinCount - 1
				}
				neutralHueHistogram[bin]++
				neutralPixels++
			}
		}
	}

	meanR := sumR / totalPixels
	meanG := sumG / totalPixels
	meanB := sumB / totalPixels

	flags := 0
	hasEnoughNeutral := float64(neutralPixels)/totalPixels >= neutralPixelMinRatio

	// Neutral-pixel RGB cast: pixels that should be gray reveal a cast when their mean shifts off-gray.
	if hasEnoughNeutral {
		nR := neutralSumR / float64(neutralPixels)
		nG := neutralSumG / float64(neutralPixels)
		nB := neutralSumB / float64(neutralPixels)
		gray := (nR + nG + nB) / 3.0
		maxDeviation := math.Max(math.Abs(nR-gray), math.Max(math.Abs(nG-gray), math.Abs(nB-gray)))
		if maxDeviation > neutralCastThreshold {
			flags++
		}
	}

	// Neutral-pixel hue skew: balanced images distribute neutral-pixel hues uniformly; a cast concentrates them.
	if hasEnoughNeutral {
		maxBin := 0
		for _, c := range neutralHueHistogram {
			if c > maxBin {
				maxBin = c
			}
		}
		if float64(maxBin)/float64(neutralPixels) > neutralHueSkewThreshold {
			flags++
		}
	}

	// White Balance Score (von Kries)
	if meanG > 0 {
		deviation := math.Abs(meanR/meanG-1.0) + math.Abs(meanB/meanG-1.0)
		if deviation > whiteBalanceThreshold {
			flags++
		}
	}

	return flags >= 2
}

func shouldUpscale(input *types.ImageData) bool {
	const _2Mp = 4_194_304

	bounds := input.Pixels.Bounds()
	mp := bounds.Dx() * bounds.Dy()

	return mp <= _2Mp
}

// endregion
