package utils

import (
	"fmt"
	"image"
	"image/color"
	"math"
)

// opaqueAlpha is the weight at or above which a blend is indistinguishable from a straight write: the result is
// rounded to 8 bits, so anything this close to 1 lands on the source byte anyway. Naming it keeps the threshold in the
// two places that test it from drifting apart.
const opaqueAlpha = 0.999

// ParamIntensity is the map key used by operations to carry the per-run blend amount to Model.Run, decoupled from the
// operation Id so the registry reuses a single session across all intensities.
const ParamIntensity = "intensity"

// IntensityFromParams reads the per-run blend amount from a params map; it defaults to 1.0 (full model output) when the
// key is absent or has the wrong type.
func IntensityFromParams(params map[string]any) float32 {
	if v, ok := params[ParamIntensity].(float32); ok {
		return v
	}
	return 1.0
}

// IntensityCacheKey is the stable per-run signature folded into the image cache key so that runs with the same Id but a
// different blend amount do not collide.
func IntensityCacheKey(intensity float32) string {
	return fmt.Sprintf("i=%.3g", intensity)
}

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

	// Fast path: when both inputs are concrete RGBA-family buffers, blend via direct Pix indexing. Sample16 reproduces
	// the exact 16-bit values At().RGBA() would return, so the output is bit-identical to the generic path while
	// avoiding per-pixel interface dispatch on two sources.
	//
	// No origin offset appears below: RgbPixBuffer hands back each image's own Pix, which already starts at that
	// image's Bounds().Min, so 0-based indexing is correct whatever the bounds are.
	oPix, oStride, oFast := RgbPixBuffer(original)
	mPix, mStride, mFast := RgbPixBuffer(modelOutput)
	_, oIsNRGBA := original.(*image.NRGBA)
	_, mIsNRGBA := modelOutput.(*image.NRGBA)

	// Both loops are sized from original but index both buffers, so a modelOutput smaller than the original would run
	// off the end of mPix. The generic path reads such pixels as transparent black rather than panicking, so leave the
	// mismatch to it.
	modelBounds := modelOutput.Bounds()
	modelCovers := modelBounds.Dx() >= width && modelBounds.Dy() >= height

	if oFast && mFast && modelCovers {
		for y := range height {
			oRow := y * oStride
			mRow := y * mStride
			dst := y * result.Stride
			for x := range width {
				origR, origG, origB, origA := Sample16(oPix, oRow+x*4, oIsNRGBA)
				modelR, modelG, modelB, _ := Sample16(mPix, mRow+x*4, mIsNRGBA)

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

	// At() is absolute, so unlike the Pix path above this one does need each image's origin added back.
	origMin, modelMin := bounds.Min, modelBounds.Min

	for y := range height {
		for x := range width {
			origR, origG, origB, origA := original.At(origMin.X+x, origMin.Y+y).RGBA()
			modelR, modelG, modelB, _ := modelOutput.At(modelMin.X+x, modelMin.Y+y).RGBA()

			// Convert to float32 in range [0, 255]
			oR := float32(origR) / 257.0
			oG := float32(origG) / 257.0
			oB := float32(origB) / 257.0

			mR := float32(modelR) / 257.0
			mG := float32(modelG) / 257.0
			mB := float32(modelB) / 257.0

			// Extrapolates past the endpoints for a negative intensity, which is what inverts the effect.
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

// blendTileWithOverlap blends a tile into a destination image, ramping the edges it shares with tiles already written.
//
// overlapX and overlapY are how many pixels this tile actually overlaps its left and top neighbours; zero means there
// is no neighbour on that side and that edge is written straight. They are passed in rather than assumed because the
// grid shifts a final tile back to sit flush with the image instead of shrinking it, so the last column and row
// overlap by much more than the configured overlap - see TileGrid.OverlapsX.
//
// The ramp is a raised cosine rather than a straight line. Both are continuous, but a linear ramp's slope jumps at
// each end of the band, and on smooth gradients that reads as a visible edge where the blend starts and stops. The
// same curve is used by the diffusion upscaler's own accumulator, for the same reason.
//
// dst is always an *image.RGBA. When src is a concrete RGBA-family type, it is blended via direct Pix indexing instead
// of the image.Image interface, and the fully opaque interior - past both ramps, where the weight is exactly 1 - is
// copied a row at a time. This is output-identical to the per-pixel path but avoids interface dispatch and the float
// blend across what is the overwhelming majority of every tile.
func blendTileWithOverlap(dst *image.RGBA, src image.Image, x, y, overlapX, overlapY int) {
	srcBounds := src.Bounds()

	dstBounds := dst.Bounds()

	// Pre-calculate actual rendering bounds to avoid repeated checks
	maxX := min(dstBounds.Dx()-x, srcBounds.Dx())
	maxY := min(dstBounds.Dy()-y, srcBounds.Dy())
	if maxX <= 0 || maxY <= 0 {
		return
	}

	// A ramp wider than half the tile would have its two ends overlap each other; clamping matches what the diffusion
	// upscaler's edgeWeights does and keeps each ramp monotonic.
	rampX := min(overlapX, maxX/2)
	rampY := min(overlapY, maxY/2)

	srcPix, srcStride, srcFast := RgbPixBuffer(src)
	_, srcIsNRGBA := src.(*image.NRGBA)

	// The bulk path reproduces the per-pixel one only for a premultiplied buffer: an NRGBA tile with a non-opaque
	// pixel needs the premultiply below, which a straight copy would skip.
	bulk := srcFast && !srcIsNRGBA

	for dy := range maxY {
		dstY := y + dy
		srcY := srcBounds.Min.Y + dy

		topAlpha := rampWeight(dy, rampY)

		// The horizontal ramp only affects the leftmost rampX columns; everything to their right shares this row's
		// weight - 1, or topAlpha while the row is still inside the top band.
		dstRow := dst.PixOffset(x, dstY)

		if bulk && topAlpha >= opaqueAlpha {
			// Past both ramps the tile simply replaces what is underneath, so the run is a copy. The alpha byte is
			// forced afterwards rather than taken from the source, matching what the per-pixel path writes.
			from := dstRow + rampX*4
			to := dstRow + maxX*4
			si := dy*srcStride + rampX*4

			copy(dst.Pix[from:to], srcPix[si:si+(maxX-rampX)*4])

			for p := from + 3; p < to; p += 4 {
				dst.Pix[p] = 255
			}

			if rampX == 0 {
				continue
			}

			blendRun(dst, srcPix, srcStride, src, srcBounds, dstRow, dy, srcY, 0, rampX, rampX, topAlpha,
				srcFast, srcIsNRGBA)

			continue
		}

		blendRun(dst, srcPix, srcStride, src, srcBounds, dstRow, dy, srcY, 0, maxX, rampX, topAlpha,
			srcFast, srcIsNRGBA)
	}
}

// blendRun blends columns [from, to) of one row. It is the per-pixel path, shared by the rows the bulk copy cannot
// take and by the left ramp of the rows it can.
func blendRun(
	dst *image.RGBA,
	srcPix []uint8,
	srcStride int,
	src image.Image,
	srcBounds image.Rectangle,
	dstRow, dy, srcY, from, to, rampX int,
	topAlpha float64,
	srcFast, srcIsNRGBA bool,
) {
	for dx := from; dx < to; dx++ {
		// The two ramps multiply where they meet, so a corner covered by both is weighted by each.
		alpha := topAlpha
		if dx < rampX {
			alpha *= rampWeight(dx, rampX)
		}

		var sr, sg, sb uint32
		if srcFast {
			// srcPix already starts at src.Bounds().Min, so this indexes from the tile's own origin. Adding
			// srcBounds.Min back - as the At() fallback below correctly must - would apply it twice.
			si := dy*srcStride + dx*4
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
		if alpha >= opaqueAlpha {
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

// rampWeight is the incoming tile's weight at offset i into a ramp of the given width, and 1 once past it (a zero
// width being "no neighbour on this side").
//
// The half-pixel offset keeps the first weight strictly positive: at i = 0 a bare cosine would be exactly 0, throwing
// away the incoming tile's outermost column entirely rather than mixing it.
func rampWeight(i, width int) float64 {
	if width <= 0 || i >= width {
		return 1
	}

	return 0.5 - 0.5*math.Cos(math.Pi*(float64(i)+0.5)/float64(width))
}
