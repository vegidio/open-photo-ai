package utils

import (
	"image"
)

// ImageToCHW is a common implementation for converting an image to a tensor
// useOffset: if true, uses bounds.Min offset; if false, assumes (0,0) origin
// standardize: if true, normalizes to [-1, 1]; if false, normalizes to [0, 1]
//
// For the common *image.NRGBA / *image.RGBA inputs it indexes the backing Pix slice directly instead of going through
// the image.Image interface (img.At().RGBA()) per pixel. The fast path reconstructs the exact 16-bit values that RGBA()
// would return, so the output is bit-identical to the generic path for any alpha while avoiding interface dispatch and
// color boxing across millions of pixels.
func ImageToCHW(img image.Image, useOffset, standardize bool) []float32 {
	bounds := img.Bounds()
	width, height := bounds.Dx(), bounds.Dy()
	tensor := make([]float32, 3*height*width)

	// Per-channel plane base offsets, hoisted out of the inner loop.
	plane := height * width
	gBase, bBase := plane, 2*plane

	// Fast path: read the backing pixel buffer directly for the concrete RGBA-family types. It indexes
	// relative to Bounds().Min, which equals the generic path when useOffset is true, or when the
	// origin is already (0,0) (the case for every imaging.Resize/Crop output passed with useOffset).
	atOrigin := bounds.Min.X == 0 && bounds.Min.Y == 0
	if pix, stride, ok := rgbPixBuffer(img); ok && (useOffset || atOrigin) {
		_, isNRGBA := img.(*image.NRGBA)

		for y := 0; y < height; y++ {
			row := y * stride // bounds.Min already subtracted: Pix/Stride are relative to Min
			dst := y * width

			for x := 0; x < width; x++ {
				// Reconstruct the exact 16-bit values color.RGBA/NRGBA.RGBA() returns.
				r16, g16, b16, _ := sample16(pix, row+x*4, isNRGBA)
				writeCHW(tensor, dst, gBase, bBase, r16, g16, b16, standardize)
				dst++
			}
		}

		return tensor
	}

	// Generic fallback for any other image.Image implementation.
	for y := 0; y < height; y++ {
		dst := y * width

		for x := 0; x < width; x++ {
			var r, g, b uint32

			if useOffset {
				r, g, b, _ = img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
			} else {
				r, g, b, _ = img.At(x, y).RGBA()
			}

			writeCHW(tensor, dst, gBase, bBase, r, g, b, standardize)
			dst++
		}
	}

	return tensor
}

// CHWToImage converts a tensor in CHW format back to an image
// standardize: if true, denormalizes from [-1, 1]; if false, denormalizes from [0, 1]
// Expects the format: [1, 3, H, W] in CHW format (RGB)
func CHWToImage(data []float32, width, height int, standardize bool) image.Image {
	img := image.NewRGBA(image.Rect(0, 0, width, height))
	pix := img.Pix

	plane := height * width
	gBase, bBase := plane, 2*plane

	for y := 0; y < height; y++ {
		dst := y * img.Stride
		base := y * width

		for x := 0; x < width; x++ {
			idx := base + x
			var r, g, b float32

			if standardize {
				// Denormalize from [-1, 1] to [0, 255]
				r = Clamp255((data[idx]*0.5 + 0.5) * 255.0)
				g = Clamp255((data[gBase+idx]*0.5 + 0.5) * 255.0)
				b = Clamp255((data[bBase+idx]*0.5 + 0.5) * 255.0)
			} else {
				// Denormalize from [0, 1] to [0, 255]
				r = Clamp255(data[idx] * 255.0)
				g = Clamp255(data[gBase+idx] * 255.0)
				b = Clamp255(data[bBase+idx] * 255.0)
			}

			pix[dst] = uint8(r)
			pix[dst+1] = uint8(g)
			pix[dst+2] = uint8(b)
			pix[dst+3] = 255
			dst += 4
		}
	}

	return img
}

// writeCHW writes one pixel's three channels into the CHW tensor using the 16-bit (0-65535) channel
// values, matching the normalization used by both the fast and generic paths.
func writeCHW(tensor []float32, idx, gBase, bBase int, r, g, b uint32, standardize bool) {
	if standardize {
		// Convert to RGB, normalize to [-1, 1]
		tensor[idx] = (float32(r/257)/255.0 - 0.5) / 0.5
		tensor[gBase+idx] = (float32(g/257)/255.0 - 0.5) / 0.5
		tensor[bBase+idx] = (float32(b/257)/255.0 - 0.5) / 0.5
	} else {
		// Normalize to [0, 1]
		tensor[idx] = float32(r) / 65535.0
		tensor[gBase+idx] = float32(g) / 65535.0
		tensor[bBase+idx] = float32(b) / 65535.0
	}
}
