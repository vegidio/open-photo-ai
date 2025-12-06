package upscale

import (
	"image"
	"image/color"
)

// blendTileWithOverlap blends a tile into a destination image with soft blending in overlap regions
func blendTileWithOverlap(dst *image.RGBA, src image.Image, x, y, overlap int, blendLeft, blendTop bool) {
	srcBounds := src.Bounds()
	dstBounds := dst.Bounds()

	srcWidth := srcBounds.Dx()
	srcHeight := srcBounds.Dy()

	// Pre-calculate actual rendering bounds to avoid repeated checks
	maxX := min(dstBounds.Dx()-x, srcWidth)
	maxY := min(dstBounds.Dy()-y, srcHeight)

	overlapFloat := float64(overlap)

	for dy := 0; dy < maxY; dy++ {
		dstY := y + dy
		srcY := srcBounds.Min.Y + dy

		// Calculate vertical alpha
		topAlpha := 1.0
		if blendTop && dy < overlap {
			topAlpha = float64(dy) / overlapFloat
		}

		for dx := 0; dx < maxX; dx++ {
			dstX := x + dx
			srcX := srcBounds.Min.X + dx

			// Calculate horizontal alpha
			alpha := calculateBlendAlpha(dx, dy, overlap, overlapFloat, topAlpha, blendLeft, blendTop)

			srcColor := src.At(srcX, srcY)

			if alpha >= 0.999 {
				// No blending needed - direct copy
				dst.Set(dstX, dstY, srcColor)
			} else {
				// Blend colors
				blendPixel(dst, dstX, dstY, srcColor, alpha)
			}
		}
	}
}

// calculateBlendAlpha computes the blend weight based on position in the overlap region
func calculateBlendAlpha(dx, dy, overlap int, overlapFloat, topAlpha float64, blendLeft, blendTop bool) float64 {
	alpha := 1.0

	if blendLeft && dx < overlap {
		alpha = float64(dx) / overlapFloat

		// Corner region: multiply both weights
		if blendTop && dy < overlap {
			alpha *= topAlpha
		}
	} else if blendTop && dy < overlap {
		alpha = topAlpha
	}

	return alpha
}

// blendPixel blends source color with destination pixel at the given alpha
func blendPixel(dst *image.RGBA, x, y int, srcColor color.Color, alpha float64) {
	dstColor := dst.At(x, y)
	sr, sg, sb, _ := srcColor.RGBA()
	dr, dg, db, _ := dstColor.RGBA()

	invAlpha := 1.0 - alpha

	// RGBA() returns 16-bit values, so shift by 8 to get 8-bit
	r := uint8(float64(sr>>8)*alpha + float64(dr>>8)*invAlpha)
	g := uint8(float64(sg>>8)*alpha + float64(dg>>8)*invAlpha)
	b := uint8(float64(sb>>8)*alpha + float64(db>>8)*invAlpha)

	dst.Set(x, y, color.RGBA{R: r, G: g, B: b, A: 255})
}
