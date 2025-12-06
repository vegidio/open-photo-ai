package utils

import (
	"image"
	"image/color"
)

// ImageToCHW is a common implementation for converting an image to a tensor
// useOffset: if true, uses bounds.Min offset; if false, assumes (0,0) origin
// standardize: if true, normalizes to [-1, 1]; if false, normalizes to [0, 1]
func ImageToCHW(img image.Image, useOffset, standardize bool) []float32 {
	bounds := img.Bounds()
	width, height := bounds.Dx(), bounds.Dy()
	tensor := make([]float32, 3*height*width)

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			var r, g, b uint32
			if useOffset {
				r, g, b, _ = img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
			} else {
				r, g, b, _ = img.At(x, y).RGBA()
			}

			idx := y*width + x
			if standardize {
				// Convert to RGB, normalize to [-1, 1]
				tensor[0*height*width+idx] = (float32(r/257)/255.0 - 0.5) / 0.5
				tensor[1*height*width+idx] = (float32(g/257)/255.0 - 0.5) / 0.5
				tensor[2*height*width+idx] = (float32(b/257)/255.0 - 0.5) / 0.5
			} else {
				// Normalize to [0, 1]
				tensor[0*height*width+idx] = float32(r) / 65535.0
				tensor[1*height*width+idx] = float32(g) / 65535.0
				tensor[2*height*width+idx] = float32(b) / 65535.0
			}
		}
	}

	return tensor
}

// CHWToImage converts a tensor in CHW format back to an image
// standardize: if true, denormalizes from [-1, 1]; if false, denormalizes from [0, 1]
// Expects the format: [1, 3, H, W] in CHW format (RGB)
func CHWToImage(data []float32, width, height int, standardize bool) image.Image {
	img := image.NewRGBA(image.Rect(0, 0, width, height))

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			idx := y*width + x
			var r, g, b float32
			if standardize {
				// Denormalize from [-1, 1] to [0, 255]
				r = ClampFloat32((data[0*height*width+idx]*0.5 + 0.5) * 255.0)
				g = ClampFloat32((data[1*height*width+idx]*0.5 + 0.5) * 255.0)
				b = ClampFloat32((data[2*height*width+idx]*0.5 + 0.5) * 255.0)
			} else {
				// Denormalize from [0, 1] to [0, 255]
				r = ClampFloat32(data[0*height*width+idx] * 255.0)
				g = ClampFloat32(data[1*height*width+idx] * 255.0)
				b = ClampFloat32(data[2*height*width+idx] * 255.0)
			}

			img.Set(x, y, color.RGBA{R: uint8(r), G: uint8(g), B: uint8(b), A: 255})
		}
	}

	return img
}
