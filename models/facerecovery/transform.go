package facerecovery

import (
	"image"
	"image/color"
	"math"

	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
)

// invertAffine computes the inverse of a 2x3 affine transformation matrix
func invertAffine(transform AffineMatrix) AffineMatrix {
	a, b, c := transform[0][0], transform[0][1], transform[0][2]
	d, e, f := transform[1][0], transform[1][1], transform[1][2]

	det := a*e - b*d

	// Check for near-zero determinant to avoid numerical issues
	if det == 0 || (det > -1e-10 && det < 1e-10) {
		// Return identity matrix
		return AffineMatrix{{1, 0, 0}, {0, 1, 0}}
	}

	invDet := 1.0 / det

	return AffineMatrix{
		{e * invDet, -b * invDet, (b*f - c*e) * invDet},
		{-d * invDet, a * invDet, (c*d - a*f) * invDet},
	}
}

// warpAffine applies an affine transformation to an image
func warpAffine(img image.Image, transform AffineMatrix, width, height int) image.Image {
	result := imaging.New(width, height, color.NRGBA{})
	bounds := img.Bounds()

	// Compute inverse transform to map from destination to source
	invTransform := invertAffine(transform)

	// Get direct access to pixel buffer for faster writes
	pix := result.Pix
	stride := result.Stride

	// Cache matrix values to avoid repeated array lookups
	m00, m01, m02 := invTransform[0][0], invTransform[0][1], invTransform[0][2]
	m10, m11, m12 := invTransform[1][0], invTransform[1][1], invTransform[1][2]

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			// Apply inverse transform - using cached values instead of array lookups
			srcX := m00*float32(x) + m01*float32(y) + m02
			srcY := m10*float32(x) + m11*float32(y) + m12

			// Bilinear interpolation with reflection padding
			col := bilinearInterpolate(img, srcX, srcY, bounds, true)

			// Convert to NRGBA - bilinearInterpolate returns color.Color
			nrgba := col.(color.NRGBA)

			// Write directly to the pixel buffer
			i := y*stride + x*4
			pix[i+0] = nrgba.R
			pix[i+1] = nrgba.G
			pix[i+2] = nrgba.B
			pix[i+3] = nrgba.A
		}
	}

	return result
}

// bilinearInterpolate performs bilinear interpolation at floating point coordinates
func bilinearInterpolate(img image.Image, x, y float32, bounds image.Rectangle, reflect bool) color.Color {
	// Calculate integer coordinates of the four surrounding pixels
	x0 := int(math.Floor(float64(x)))
	y0 := int(math.Floor(float64(y)))
	x1 := x0 + 1
	y1 := y0 + 1

	// Calculate interpolation weights and their complements
	wx := x - float32(x0)
	wy := y - float32(y0)
	wx0 := 1.0 - wx
	wy0 := 1.0 - wy

	// Helper function to get pixel with boundary handling
	getPixel := func(px, py int) color.Color {
		if reflect {
			px = reflectCoord(px, bounds.Min.X, bounds.Max.X)
			py = reflectCoord(py, bounds.Min.Y, bounds.Max.Y)
		} else {
			px = utils.ClampInt(px, bounds.Min.X, bounds.Max.X-1)
			py = utils.ClampInt(py, bounds.Min.Y, bounds.Max.Y-1)
		}
		return img.At(px, py)
	}

	// Get the four corner pixels
	c00 := getPixel(x0, y0)
	c10 := getPixel(x1, y0)
	c01 := getPixel(x0, y1)
	c11 := getPixel(x1, y1)

	// Extract RGBA components (returns 16-bit values 0-65535)
	r00, g00, b00, a00 := c00.RGBA()
	r10, g10, b10, a10 := c10.RGBA()
	r01, g01, b01, a01 := c01.RGBA()
	r11, g11, b11, a11 := c11.RGBA()

	// Two-step lerp: first along x-axis, then along y-axis
	lerp2D := func(v00, v01, v10, v11 uint32) uint8 {
		// Interpolate along x-axis for both rows
		v0 := float32(v00)*wx0 + float32(v10)*wx
		v1 := float32(v01)*wx0 + float32(v11)*wx

		// Interpolate along y-axis
		result := v0*wy0 + v1*wy
		return uint8(result / 257) // Convert from 16-bit to 8-bit
	}

	return color.NRGBA{
		R: lerp2D(r00, r01, r10, r11),
		G: lerp2D(g00, g01, g10, g11),
		B: lerp2D(b00, b01, b10, b11),
		A: lerp2D(a00, a01, a10, a11),
	}
}

func reflectCoord(coord, min, max int) int {
	size := max - min
	coord -= min

	if coord < 0 {
		coord = -coord - 1
	}

	if coord >= size {
		coord = 2*size - coord - 1
	}

	return coord + min
}
