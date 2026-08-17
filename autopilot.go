package opai

import (
	"context"
	"image"
	"math"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// SuggestEnhancements analyzes the input image and returns a list of recommended enhancements.
//
// The face-detection path may trigger a model download on first use; pass a cancellable ctx to abort it.
func SuggestEnhancements(ctx context.Context, input *types.ImageData) []types.ModelType {
	enhancementTypes := make([]types.ModelType, 0)

	if shouldFaceRecovery(ctx, input) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeFaceRecovery)
	}

	// The light and color heuristics both need per-pixel statistics over the whole image, so they share one scan
	// rather than walking every pixel twice.
	stats := scanImage(input.Pixels)

	if shouldLightAdjustment(stats) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeLightAdjustment)
	}

	if shouldColorBalance(stats) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeColorBalance)
	}

	if shouldUpscale(input) {
		enhancementTypes = append(enhancementTypes, types.ModelTypeUpscale)
	}

	internal.Log().Info("suggested enhancements", "count", len(enhancementTypes), "types", enhancementTypes)
	return enhancementTypes
}

// region - Private functions

func shouldFaceRecovery(ctx context.Context, input *types.ImageData) bool {
	model, lease, err := facerecovery.GetDtModel(ctx, types.ExecutionProviderAuto)
	if err != nil {
		internal.Log().Warn("face detection model unavailable; skipping face-recovery suggestion", "err", err)
		return false
	}

	// Keeps the detection model alive for the extraction below.
	defer lease.Release()

	faces, err := facerecovery.ExtractFaces(ctx, model, input.Pixels, nil)
	if err != nil {
		internal.Log().Warn("face detection failed; skipping face-recovery suggestion", "err", err)
		return false
	}

	return len(faces) > 0
}

// Thresholds that decide what the scan accumulates, as opposed to how the accumulations are judged (those stay with
// their respective predicates below).
const (
	darkThreshold    = 15  // 8-bit luminance considered near black
	brightThreshold  = 240 // 8-bit luminance considered near white
	neutralSatCutoff = 40.0
	hueBinCount      = 12
)

// imageStats holds everything the light-adjustment and colour-balance heuristics need from the image, gathered in a
// single pass.
type imageStats struct {
	totalPixels float64

	// Luminance distribution (Rec. 709, 8-bit)
	sumLuminance float64
	darkPixels   int
	brightPixels int

	// Channel means and the near-neutral pixels that reveal a colour cast
	sumR, sumG, sumB                      float64
	neutralSumR, neutralSumG, neutralSumB float64
	neutralPixels                         int
	neutralHueHistogram                   [hueBinCount]int
}

// scanImage walks the image once and accumulates the statistics both colour heuristics need.
//
// For the concrete RGBA-family types it indexes the backing pixel buffer directly rather than going through
// image.Image.At().RGBA() per pixel. utils.Sample16 reconstructs the exact 16-bit values At() would have returned, so
// the statistics — and therefore the suggestions — are identical to the generic path on every image.
func scanImage(img image.Image) imageStats {
	bounds := img.Bounds()
	width, height := bounds.Dx(), bounds.Dy()

	stats := imageStats{totalPixels: float64(width * height)}
	if stats.totalPixels == 0 {
		return stats
	}

	accumulate := func(r, g, b uint32) {
		// Convert from 16-bit to 8-bit
		r8 := float64(r >> 8)
		g8 := float64(g >> 8)
		b8 := float64(b >> 8)

		// Perceived luminance (Rec. 709)
		luminance := 0.2126*r8 + 0.7152*g8 + 0.0722*b8
		stats.sumLuminance += luminance

		if luminance <= darkThreshold {
			stats.darkPixels++
		} else if luminance >= brightThreshold {
			stats.brightPixels++
		}

		stats.sumR += r8
		stats.sumG += g8
		stats.sumB += b8

		h, s, _ := utils.RgbToHsv(r8, g8, b8)

		if s < neutralSatCutoff {
			stats.neutralSumR += r8
			stats.neutralSumG += g8
			stats.neutralSumB += b8

			bin := int(h / 360.0 * float64(hueBinCount))
			if bin >= hueBinCount {
				bin = hueBinCount - 1
			}
			stats.neutralHueHistogram[bin]++
			stats.neutralPixels++
		}
	}

	// Fast path: read the backing pixel buffer directly, skipping the per-pixel interface dispatch and color boxing.
	if pix, stride, ok := utils.RgbPixBuffer(img); ok {
		_, isNRGBA := img.(*image.NRGBA)

		for y := range height {
			row := y * stride // Pix/Stride are already relative to Bounds().Min

			for x := range width {
				r, g, b, _ := utils.Sample16(pix, row+x*4, isNRGBA)
				accumulate(r, g, b)
			}
		}

		return stats
	}

	// Generic fallback for any other image.Image implementation.
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			r, g, b, _ := img.At(x, y).RGBA()
			accumulate(r, g, b)
		}
	}

	return stats
}

func shouldLightAdjustment(stats imageStats) bool {
	if stats.totalPixels == 0 {
		return false
	}

	const (
		meanDarkLimit   = 50
		meanBrightLimit = 200
		clippingRatio   = 0.35
	)

	meanLuminance := stats.sumLuminance / stats.totalPixels
	darkRatio := float64(stats.darkPixels) / stats.totalPixels
	brightRatio := float64(stats.brightPixels) / stats.totalPixels

	if meanLuminance < meanDarkLimit && darkRatio > clippingRatio {
		return true
	}

	if meanLuminance > meanBrightLimit && brightRatio > clippingRatio {
		return true
	}

	return false
}

func shouldColorBalance(stats imageStats) bool {
	if stats.totalPixels == 0 {
		return false
	}

	const (
		neutralPixelMinRatio    = 0.02
		neutralCastThreshold    = 12.0
		neutralHueSkewThreshold = 0.45
		whiteBalanceThreshold   = 0.5
	)

	neutralPixels := stats.neutralPixels
	neutralSumR, neutralSumG, neutralSumB := stats.neutralSumR, stats.neutralSumG, stats.neutralSumB
	neutralHueHistogram := stats.neutralHueHistogram

	meanR := stats.sumR / stats.totalPixels
	meanG := stats.sumG / stats.totalPixels
	meanB := stats.sumB / stats.totalPixels

	flags := 0
	hasEnoughNeutral := float64(neutralPixels)/stats.totalPixels >= neutralPixelMinRatio

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
	const maxPixelsForUpscale = 4 << 20

	bounds := input.Pixels.Bounds()
	pixels := bounds.Dx() * bounds.Dy()

	return pixels <= maxPixelsForUpscale
}

// endregion
