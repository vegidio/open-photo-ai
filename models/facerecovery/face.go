package facerecovery

import (
	"context"
	"image"
	"image/color"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/detection/newyork"
	"github.com/vegidio/open-photo-ai/types"
)

const (
	// maskMarginFactor expands the face bbox (by this fraction of its longest side) so the blend window covers the
	// entire feathered mask region, which extends beyond the face itself.
	maskMarginFactor = 0.5
	// minBlendAlpha is the lowest mask alpha still worth blending; below it the contribution is imperceptible.
	minBlendAlpha = 0.001
)

// GetDtModel returns the face-detection model together with the lease that keeps it resident.
//
// The caller owns the lease and must release it once it is done with the model - deferred immediately after the error
// check. Holding the model without the lease is a use-after-free waiting to happen, which is why the two are returned
// together rather than the lease being hidden inside.
//
// precision is the caller's, not this package's, and it should be the precision of the face-recovery operation the
// faces are being detected for: a run that recovers at fp16 detects at fp16 too. It used to be pinned to fp32 here
// because no fp16 detection model was published; now that one is, pinning would mean a user on the SD tier paying
// for the 88 MB fp32 graph purely to feed a 44 MB fp16 one.
//
// The two precisions agree on what they find. Measured over 25 variants of the sample image - rotations, scales,
// flips, brightness, contrast, grayscale, crops and extreme aspect ratios - fp16 returned the same faces in the same
// order every time, differing only in sub-pixel box and landmark positions. The order matters as much as the count:
// the GUI's per-face de-selection is stored as indices into this slice, so a precision that reordered faces would
// silently re-target it.
func GetDtModel(
	ctx context.Context,
	ep types.ExecutionProvider,
	precision types.Precision,
) (types.Model[[]detection.Face], *internal.Lease, error) {
	dtOp := newyork.Op(precision)

	lease, err := internal.AcquireModel(dtOp.Id(), ep, func(ep types.ExecutionProvider) (any, error) {
		return newyork.New(ctx, dtOp, ep, nil)
	})

	if err != nil {
		return nil, nil, errors.Wrap(err, "failed to create Face Detection model")
	}

	model, ok := lease.Model().(types.Model[[]detection.Face])
	if !ok {
		lease.Release()
		return nil, nil, errors.Errorf("unexpected model type for face detection: %s", dtOp.Id())
	}

	return model, lease, nil
}

func alignFace(img image.Image, landmarks [5]detection.PointF, tileSize int) (image.Image, AffineMatrix) {
	transform := calculateSimilarityTransform(landmarks[:], detection.ArcfaceTemplate)
	aligned := warpAffine(img, transform, tileSize, tileSize)
	return aligned, transform
}

// createCircularMask creates a soft circular mask with feathered edges
// If blurSigma > 0, applies Gaussian blur with the specified sigma
func createCircularMask(width, height int, blurSigma float64) image.Image {
	const (
		innerRadius  = 0.7 // Full opacity within this radius
		outerRadius  = 1.0 // Zero opacity beyond this radius
		falloffRange = outerRadius - innerRadius
	)

	mask := imaging.New(width, height, color.NRGBA{})
	centerX := float64(width) / 2
	centerY := float64(height) / 2
	invCenterX := 1.0 / centerX
	invCenterY := 1.0 / centerY
	innerRadiusSq := innerRadius * innerRadius
	outerRadiusSq := outerRadius * outerRadius

	for y := range height {
		dy := (float64(y) - centerY) * invCenterY
		dySq := dy * dy

		for x := range width {
			dx := (float64(x) - centerX) * invCenterX
			distanceSq := dx*dx + dySq

			var alpha float64
			if distanceSq <= innerRadiusSq {
				alpha = 1.0
			} else if distanceSq <= outerRadiusSq {
				// Only calculate sqrt when needed for the falloff region
				distance := math.Sqrt(distanceSq)
				alpha = (outerRadius - distance) / falloffRange
			} else {
				alpha = 0.0
			}

			// Direct pixel buffer access for better performance
			offset := (y-mask.Rect.Min.Y)*mask.Stride + (x-mask.Rect.Min.X)*4
			mask.Pix[offset] = 255                  // R
			mask.Pix[offset+1] = 255                // G
			mask.Pix[offset+2] = 255                // B
			mask.Pix[offset+3] = uint8(alpha * 255) // A
		}
	}

	if blurSigma > 0 {
		return imaging.Blur(mask, blurSigma)
	}

	return mask
}

// blendFaceInto blends a restored face into dst, in place, using the forward affine transform.
//
// In place rather than onto a copy: this used to clone the whole image per face, so a group photo paid a full-frame
// allocation for every face in it just to write a few 512x512 regions. Each destination pixel is read (as the
// unrestored background) and written in the same iteration, so reading and writing one buffer is safe - and it
// preserves the existing behaviour of each face blending over the faces already composited before it.
func blendFaceInto(dst *image.NRGBA, restored, mask image.Image, transform AffineMatrix, bbox detection.RectF, tileSize int) {
	origBounds := dst.Bounds()

	bboxWidth := bbox.Max.X - bbox.Min.X
	bboxHeight := bbox.Max.Y - bbox.Min.Y
	margin := max(bboxWidth, bboxHeight) * maskMarginFactor

	minX := max(0, int(bbox.Min.X-margin))
	minY := max(0, int(bbox.Min.Y-margin))
	maxX := min(origBounds.Max.X, int(bbox.Max.X+margin))
	maxY := min(origBounds.Max.Y, int(bbox.Max.Y+margin))

	// Precompute transform coefficients for better performance
	a00, a01, a02 := transform[0][0], transform[0][1], transform[0][2]
	a10, a11, a12 := transform[1][0], transform[1][1], transform[1][2]

	tileSizeFloat := float32(tileSize)

	// Hoisted out of the loop: each of the three sources is sampled once per destination pixel (the mask and the
	// restored face four times each, through bilinearInterpolate), so resolving the pixel buffer once here removes
	// the bulk of the interface dispatch from the blend.
	maskSrc := newSampler(mask)
	restoredSrc := newSampler(restored)
	originalSrc := newSampler(dst)

	// Get direct access to pixel buffer for faster writes
	stride := dst.Stride
	pixels := dst.Pix

	// Blend the restored face back using forward transform for both the face and its mask. The mask lives in aligned
	// (tileSize x tileSize) space, which is exactly where the forward transform maps each destination pixel — so we
	// sample it directly instead of pre-warping it across the whole image.
	for y := minY; y < maxY; y++ {
		// Precompute y-dependent transform components
		transformXBase := a01*float32(y) + a02
		transformYBase := a11*float32(y) + a12

		rowOffset := y * stride

		for x := minX; x < maxX; x++ {
			// Apply forward transform: original coords -> aligned coords
			alignedX := a00*float32(x) + transformXBase
			alignedY := a10*float32(x) + transformYBase

			if alignedX >= 0 && alignedX < tileSizeFloat &&
				alignedY >= 0 && alignedY < tileSizeFloat {
				alpha := float32(bilinearInterpolate(maskSrc, alignedX, alignedY, false).A) / 255.0

				if alpha > minBlendAlpha {
					restoredCol := bilinearInterpolate(restoredSrc, alignedX, alignedY, false)

					or, og, ob, _ := originalSrc.at(x, y)
					rr, rg, rb, _ := restoredCol.RGBA()

					oneMinusAlpha := 1 - alpha
					finalR := uint8(float32(rr/257)*alpha + float32(or/257)*oneMinusAlpha)
					finalG := uint8(float32(rg/257)*alpha + float32(og/257)*oneMinusAlpha)
					finalB := uint8(float32(rb/257)*alpha + float32(ob/257)*oneMinusAlpha)

					pixelOffset := rowOffset + x*4
					pixels[pixelOffset] = finalR
					pixels[pixelOffset+1] = finalG
					pixels[pixelOffset+2] = finalB
					pixels[pixelOffset+3] = 255
				}
			}
		}
	}
}

// region - Private functions

// calculateSimilarityTransform computes a similarity transformation matrix from source landmarks to destination
// landmarks using the least squares fitting.
//
// This implementation uses a covariance-based approach that minimizes the sum of squared distances between transformed
// source points and destination points.
func calculateSimilarityTransform(src, dst []detection.PointF) AffineMatrix {
	numPoints := len(src)

	var srcMeanX, srcMeanY, dstMeanX, dstMeanY float32
	for i := range numPoints {
		srcMeanX += src[i].X
		srcMeanY += src[i].Y
		dstMeanX += dst[i].X
		dstMeanY += dst[i].Y
	}

	srcMeanX /= float32(numPoints)
	srcMeanY /= float32(numPoints)
	dstMeanX /= float32(numPoints)
	dstMeanY /= float32(numPoints)

	// Compute the covariance matrix components
	var sXX, sXY, sYY float32
	var dXsX, dXsY, dYsX, dYsY float32

	for i := range numPoints {
		sX := src[i].X - srcMeanX
		sY := src[i].Y - srcMeanY

		dX := dst[i].X - dstMeanX
		dY := dst[i].Y - dstMeanY

		// Source covariance
		sXX += sX * sX
		sXY += sX * sY
		sYY += sY * sY

		// Cross-covariance
		dXsX += dX * sX
		dXsY += dX * sY
		dYsX += dY * sX
		dYsY += dY * sY
	}

	srcNorm := sXX + sYY

	// Degenerate source (all landmarks coincide) → no scale/rotation can be fit; fall back to a pure translation that
	// maps the source centroid onto the destination centroid, avoiding a NaN matrix from dividing by zero.
	if srcNorm < 1e-10 {
		return AffineMatrix{
			{1, 0, dstMeanX - srcMeanX},
			{0, 1, dstMeanY - srcMeanY},
		}
	}

	// Compute rotation and scale components
	a := (dXsX + dYsY) / srcNorm // cos(θ) * scale
	b := (dYsX - dXsY) / srcNorm // sin(θ) * scale

	tx := dstMeanX - (a*srcMeanX - b*srcMeanY)
	ty := dstMeanY - (b*srcMeanX + a*srcMeanY)

	return AffineMatrix{
		{a, -b, tx},
		{b, a, ty},
	}
}

// endregion
