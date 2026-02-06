package facerecovery

import (
	"image"
	"image/color"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facedetection/newyork"
	"github.com/vegidio/open-photo-ai/types"
)

func GetFdModel(ep types.ExecutionProvider) (types.Model[[]facedetection.Face], error) {
	var err error
	fdOp := newyork.Op(types.PrecisionFp32)

	model, exists := internal.Registry[fdOp.Id()]
	if !exists {
		model, err = newyork.New(fdOp, ep, nil)
		if err != nil {
			return nil, errors.Wrap(err, "failed to create Face Detection model")
		}

		internal.Registry[fdOp.Id()] = model
	}

	return model.(types.Model[[]facedetection.Face]), nil
}

// alignFace aligns a face image using the provided landmarks and returns the aligned image and the affine
// transformation matrix.
func alignFace(img image.Image, landmarks [5]facedetection.PointF, tileSize int) (image.Image, AffineMatrix) {
	transform := calculateSimilarityTransform(landmarks[:], facedetection.ArcfaceTemplate)
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

	for y := 0; y < height; y++ {
		dy := (float64(y) - centerY) * invCenterY
		dySq := dy * dy

		for x := 0; x < width; x++ {
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

// blendFace blends a restored face back into the original image using forward affine transform
func blendFace(original, restored, mask image.Image, transform AffineMatrix, bbox facedetection.RectF, tileSize int) image.Image {
	result := imaging.Clone(original)
	origBounds := original.Bounds()

	// Warp the mask to original image coordinates to get proper blur in original space
	invTransform := invertAffine(transform)
	warpedMask := warpAffine(mask, invTransform, origBounds.Dx(), origBounds.Dy())

	// Expand bbox significantly to cover the entire feathered region
	// The blurred mask extends well beyond the face, so we need a large margin
	bboxWidth := bbox.Max.X - bbox.Min.X
	bboxHeight := bbox.Max.Y - bbox.Min.Y
	margin := max(bboxWidth, bboxHeight) * 0.5

	minX := max(0, int(bbox.Min.X-margin))
	minY := max(0, int(bbox.Min.Y-margin))
	maxX := min(origBounds.Max.X, int(bbox.Max.X+margin))
	maxY := min(origBounds.Max.Y, int(bbox.Max.Y+margin))

	// Precompute transform coefficients for better performance
	a00, a01, a02 := transform[0][0], transform[0][1], transform[0][2]
	a10, a11, a12 := transform[1][0], transform[1][1], transform[1][2]

	tileSizeFloat := float32(tileSize)
	restoredBounds := restored.Bounds()

	// Get direct access to pixel buffer for faster writes
	stride := result.Stride
	pixels := result.Pix

	// Blend the restored face back using forward transform for face, warped mask for alpha
	for y := minY; y < maxY; y++ {
		// Precompute y-dependent transform components
		transformXBase := a01*float32(y) + a02
		transformYBase := a11*float32(y) + a12

		// Calculate row offset once per row
		rowOffset := y * stride

		for x := minX; x < maxX; x++ {
			// Get alpha from the warped mask
			_, _, _, ma := warpedMask.At(x, y).RGBA()
			alpha := float32(ma) / 65535.0

			// Blend even very low alpha values for maximum smoothness
			if alpha > 0.001 {
				// Apply forward transform: original coords -> aligned coords
				alignedX := a00*float32(x) + transformXBase
				alignedY := a10*float32(x) + transformYBase

				// Check if within the tileSize x tileSize aligned face
				if alignedX >= 0 && alignedX < tileSizeFloat &&
					alignedY >= 0 && alignedY < tileSizeFloat {
					// Sample from the restored face at aligned coordinates
					restoredCol := bilinearInterpolate(restored, alignedX, alignedY, restoredBounds, false)
					originalCol := original.At(x, y)

					or, og, ob, _ := originalCol.RGBA()
					rr, rg, rb, _ := restoredCol.RGBA()

					// Blend colors using the original formula
					oneMinusAlpha := 1 - alpha
					finalR := uint8(float32(rr/257)*alpha + float32(or/257)*oneMinusAlpha)
					finalG := uint8(float32(rg/257)*alpha + float32(og/257)*oneMinusAlpha)
					finalB := uint8(float32(rb/257)*alpha + float32(ob/257)*oneMinusAlpha)

					// Write directly to pixel buffer (RGBA format)
					pixelOffset := rowOffset + x*4
					pixels[pixelOffset] = finalR
					pixels[pixelOffset+1] = finalG
					pixels[pixelOffset+2] = finalB
					pixels[pixelOffset+3] = 255
				}
			}
		}
	}

	return result
}

// region - Private functions

// calculateSimilarityTransform computes a similarity transformation matrix from source landmarks to destination
// landmarks using the least squares fitting.
//
// This implementation uses a covariance-based approach that minimizes the sum of squared distances between transformed
// source points and destination points.
func calculateSimilarityTransform(src, dst []facedetection.PointF) AffineMatrix {
	numPoints := len(src)

	// Compute means
	var srcMeanX, srcMeanY, dstMeanX, dstMeanY float32
	for i := 0; i < numPoints; i++ {
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

	for i := 0; i < numPoints; i++ {
		// Centered source points
		sX := src[i].X - srcMeanX
		sY := src[i].Y - srcMeanY

		// Centered destination points
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

	// Compute rotation and scale components
	a := (dXsX + dYsY) / srcNorm // cos(θ) * scale
	b := (dYsX - dXsY) / srcNorm // sin(θ) * scale

	// Compute translation
	tx := dstMeanX - (a*srcMeanX - b*srcMeanY)
	ty := dstMeanY - (b*srcMeanX + a*srcMeanY)

	// Construct the 2x3 affine transformation matrix
	return AffineMatrix{
		{a, -b, tx},
		{b, a, ty},
	}
}

// endregion
