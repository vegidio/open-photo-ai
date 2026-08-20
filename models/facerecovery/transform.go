package facerecovery

import (
	"image"
	"image/color"
	"math"

	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
)

// invertAffine inverts a 2x3 affine matrix, returning the identity when it is singular.
func invertAffine(transform AffineMatrix) AffineMatrix {
	a, b, c := transform[0][0], transform[0][1], transform[0][2]
	d, e, f := transform[1][0], transform[1][1], transform[1][2]

	det := a*e - b*d

	// Check for near-zero determinant to avoid numerical issues
	if det == 0 || (det > -1e-10 && det < 1e-10) {
		return AffineMatrix{{1, 0, 0}, {0, 1, 0}}
	}

	invDet := 1.0 / det

	return AffineMatrix{
		{e * invDet, -b * invDet, (b*f - c*e) * invDet},
		{-d * invDet, a * invDet, (c*d - a*f) * invDet},
	}
}

// sampler reads 16-bit RGBA channel values from an image, using direct Pix indexing for the concrete RGBA-family
// types and falling back to At() for anything else. Building it once outside a loop is the whole point:
// bilinearInterpolate reads four pixels per call, so it multiplies whatever the per-pixel dispatch cost is by four.
type sampler struct {
	img     image.Image
	bounds  image.Rectangle
	pix     []uint8
	stride  int
	fast    bool
	isNRGBA bool
}

func newSampler(img image.Image) *sampler {
	pix, stride, fast := utils.RgbPixBuffer(img)
	_, isNRGBA := img.(*image.NRGBA)

	return &sampler{
		img:     img,
		bounds:  img.Bounds(),
		pix:     pix,
		stride:  stride,
		fast:    fast,
		isNRGBA: isNRGBA,
	}
}

// at returns exactly what img.At(px, py).RGBA() would, for coordinates already reflected or clamped into bounds.
// Sample16 is bit-identical to RGBA(), so the choice of path never changes the result.
func (s *sampler) at(px, py int) (r, g, b, a uint32) {
	if s.fast {
		return utils.Sample16(s.pix, (py-s.bounds.Min.Y)*s.stride+(px-s.bounds.Min.X)*4, s.isNRGBA)
	}

	return s.img.At(px, py).RGBA()
}

func warpAffine(img image.Image, transform AffineMatrix, width, height int) image.Image {
	result := imaging.New(width, height, color.NRGBA{})

	// Compute inverse transform to map from destination to source
	invTransform := invertAffine(transform)

	// Get direct access to pixel buffer for faster writes
	pix := result.Pix
	stride := result.Stride

	// Hoisted out of the loop: every destination pixel samples the same source image, so the buffer lookup and the
	// concrete-type check only need to happen once.
	src := newSampler(img)

	// Cache matrix values to avoid repeated array lookups
	m00, m01, m02 := invTransform[0][0], invTransform[0][1], invTransform[0][2]
	m10, m11, m12 := invTransform[1][0], invTransform[1][1], invTransform[1][2]

	for y := range height {
		for x := range width {
			// Destination coords -> source coords. Kept as a single expression rather than hoisting the y terms out
			// of the row: float addition is not associative, so regrouping these could shift a result by one ulp and
			// change an output pixel, which would defeat the point of the fast path below.
			srcX := m00*float32(x) + m01*float32(y) + m02
			srcY := m10*float32(x) + m11*float32(y) + m12

			// Bilinear interpolation with reflection padding
			nrgba := bilinearInterpolate(src, srcX, srcY, true)

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
func bilinearInterpolate(src *sampler, x, y float32, reflect bool) color.NRGBA {
	bounds := src.bounds

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

	// Map the four sample coordinates into bounds with the requested padding mode
	if reflect {
		x0 = reflectCoord(x0, bounds.Min.X, bounds.Max.X)
		x1 = reflectCoord(x1, bounds.Min.X, bounds.Max.X)
		y0 = reflectCoord(y0, bounds.Min.Y, bounds.Max.Y)
		y1 = reflectCoord(y1, bounds.Min.Y, bounds.Max.Y)
	} else {
		x0 = utils.ClampInt(x0, bounds.Min.X, bounds.Max.X-1)
		x1 = utils.ClampInt(x1, bounds.Min.X, bounds.Max.X-1)
		y0 = utils.ClampInt(y0, bounds.Min.Y, bounds.Max.Y-1)
		y1 = utils.ClampInt(y1, bounds.Min.Y, bounds.Max.Y-1)
	}

	// Extract RGBA components (returns 16-bit values 0-65535)
	r00, g00, b00, a00 := src.at(x0, y0)
	r10, g10, b10, a10 := src.at(x1, y0)
	r01, g01, b01, a01 := src.at(x0, y1)
	r11, g11, b11, a11 := src.at(x1, y1)

	// Two-step lerp: first along x-axis, then along y-axis
	lerp2D := func(v00, v01, v10, v11 uint32) uint8 {
		v0 := float32(v00)*wx0 + float32(v10)*wx
		v1 := float32(v01)*wx0 + float32(v11)*wx

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
