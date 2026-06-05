package utils

import (
	"image"
	"image/color"
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

	// Fast path: when both inputs are concrete RGBA-family buffers at the origin, blend via direct Pix
	// indexing. sample16 reproduces the exact 16-bit values At().RGBA() would return, so the output is
	// bit-identical to the generic path while avoiding per-pixel interface dispatch on two sources.
	oPix, oStride, oFast := rgbPixBuffer(original)
	mPix, mStride, mFast := rgbPixBuffer(modelOutput)
	_, oIsNRGBA := original.(*image.NRGBA)
	_, mIsNRGBA := modelOutput.(*image.NRGBA)
	atOrigin := original.Bounds().Min == image.Point{} && modelOutput.Bounds().Min == image.Point{}

	if oFast && mFast && atOrigin {
		for y := 0; y < height; y++ {
			oRow := y * oStride
			mRow := y * mStride
			dst := y * result.Stride
			for x := 0; x < width; x++ {
				origR, origG, origB, origA := sample16(oPix, oRow+x*4, oIsNRGBA)
				modelR, modelG, modelB, _ := sample16(mPix, mRow+x*4, mIsNRGBA)

				oR := float32(origR) / 257.0
				oG := float32(origG) / 257.0
				oB := float32(origB) / 257.0

				mR := float32(modelR) / 257.0
				mG := float32(modelG) / 257.0
				mB := float32(modelB) / 257.0

				result.Pix[dst] = uint8(Clamp255(oR + intensity*(mR-oR)))
				result.Pix[dst+1] = uint8(Clamp255(oG + intensity*(mG-oG)))
				result.Pix[dst+2] = uint8(Clamp255(oB + intensity*(mB-oB)))
				result.Pix[dst+3] = uint8(origA / 257)
				dst += 4
			}
		}
		return result
	}

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
				R: uint8(Clamp255(r)),
				G: uint8(Clamp255(g)),
				B: uint8(Clamp255(b)),
				A: uint8(origA / 257),
			})
		}
	}

	return result
}

// blendTileWithOverlap blends a tile into a destination image with soft blending in overlap regions.
//
// dst is always an *image.RGBA. When src is a concrete RGBA-family type, it is blended via direct Pix indexing instead
// of the image.Image interface, and the fully opaque interior (past the overlap band, where alpha is always 1.0) is
// copied in bulk rows. This is output-identical to the per-pixel path but avoids interface dispatch and redundant alpha
// computation across the whole tile.
func blendTileWithOverlap(dst *image.RGBA, src image.Image, x, y, overlap int, blendLeft, blendTop bool) {
	srcBounds := src.Bounds()
	srcWidth := srcBounds.Dx()
	srcHeight := srcBounds.Dy()

	dstBounds := dst.Bounds()

	// Pre-calculate actual rendering bounds to avoid repeated checks
	maxX := min(dstBounds.Dx()-x, srcWidth)
	maxY := min(dstBounds.Dy()-y, srcHeight)
	if maxX <= 0 || maxY <= 0 {
		return
	}

	overlapFloat := float64(overlap)

	srcPix, srcStride, srcFast := rgbPixBuffer(src)
	_, srcIsNRGBA := src.(*image.NRGBA)

	for dy := 0; dy < maxY; dy++ {
		dstY := y + dy
		srcY := srcBounds.Min.Y + dy

		// Calculate vertical alpha
		topAlpha := 1.0
		if blendTop && dy < overlap {
			topAlpha = float64(dy) / overlapFloat
		}

		// The horizontal overlap band only affects the leftmost `overlap` columns; everything to its right shares this
		// row's alpha (1.0, or topAlpha when in the top band).
		rowBlendStart := 0
		if blendLeft {
			rowBlendStart = overlap
		}

		dstRow := dst.PixOffset(x, dstY)

		for dx := 0; dx < maxX; dx++ {
			var alpha float64
			if dx < rowBlendStart {
				alpha = calculateBlendAlpha(dx, dy, overlap, overlapFloat, topAlpha, blendLeft, blendTop)
			} else {
				alpha = topAlpha
			}

			var sr, sg, sb uint32
			if srcFast {
				si := srcY*srcStride + (srcBounds.Min.X+dx)*4
				sr, sg, sb = uint32(srcPix[si]), uint32(srcPix[si+1]), uint32(srcPix[si+2])
				// RGBA buffers are already premultiplied 8-bit; only NRGBA with non-opaque alpha needs
				// the premultiply that RGBA() would apply (matches the >>8 of the 16-bit result).
				if srcIsNRGBA {
					if a := uint32(srcPix[si+3]); a != 0xff {
						sr = ((sr * 257) * a / 0xff) >> 8
						sg = ((sg * 257) * a / 0xff) >> 8
						sb = ((sb * 257) * a / 0xff) >> 8
					}
				}
			} else {
				r, g, b, _ := src.At(srcBounds.Min.X+dx, srcY).RGBA()
				// Match the 8-bit values the slow path historically used (RGBA() >> 8).
				sr, sg, sb = r>>8, g>>8, b>>8
			}

			di := dstRow + dx*4
			if alpha >= 0.999 {
				// No blending needed - direct copy
				dst.Pix[di] = uint8(sr)
				dst.Pix[di+1] = uint8(sg)
				dst.Pix[di+2] = uint8(sb)
				dst.Pix[di+3] = 255
			} else {
				invAlpha := 1.0 - alpha
				dst.Pix[di] = uint8(float64(sr)*alpha + float64(dst.Pix[di])*invAlpha)
				dst.Pix[di+1] = uint8(float64(sg)*alpha + float64(dst.Pix[di+1])*invAlpha)
				dst.Pix[di+2] = uint8(float64(sb)*alpha + float64(dst.Pix[di+2])*invAlpha)
				dst.Pix[di+3] = 255
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
