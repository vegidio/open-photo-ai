package utils

import (
	"fmt"
	"image"
	"image/draw"
	"math"

	ort "github.com/yalue/onnxruntime_go"
)

func ImageToNCHW(img image.Image) ([]float32, int, int) {
	rgba := image.NewRGBA(img.Bounds())
	draw.Draw(rgba, rgba.Bounds(), img, img.Bounds().Min, draw.Src)
	h, w := rgba.Bounds().Dy(), rgba.Bounds().Dx()
	data := make([]float32, 3*w*h)

	// Direct access to pixel buffer and pre-calculate constants
	pix := rgba.Pix
	stride := rgba.Stride
	planeSize := w * h
	const invScale = 1.0 / 255.0

	for y := range h {
		rowOffset := y * stride
		baseIdx := y * w
		for x := range w {
			pixelIdx := rowOffset + (x << 2) // x * 4 using bit shift
			idx := baseIdx + x

			// Read once, use three times
			r := float32(pix[pixelIdx]) * invScale
			g := float32(pix[pixelIdx+1]) * invScale
			b := float32(pix[pixelIdx+2]) * invScale

			// Sequentially writes for better cache locality
			data[idx] = r
			data[planeSize+idx] = g
			data[2*planeSize+idx] = b
		}
	}

	return data, h, w
}

func TensorToRGBA(t *ort.Tensor[float32]) (*image.RGBA, error) {
	data := t.GetData()
	shape := t.GetShape()
	if len(shape) != 4 || shape[1] != 3 {
		return nil, fmt.Errorf("unexpected tensor shape: %v", shape)
	}

	h := int(shape[2])
	w := int(shape[3])
	planeSize := w * h
	expected := 3 * planeSize

	if len(data) < expected {
		return nil, fmt.Errorf("tensor data too short: got %d, need %d", len(data), expected)
	}

	rgba := image.NewRGBA(image.Rect(0, 0, w, h))
	rPlane := data[0*planeSize : 1*planeSize]
	gPlane := data[1*planeSize : 2*planeSize]
	bPlane := data[2*planeSize : 3*planeSize]

	// Direct access to the pixel buffer for better performance
	pix := rgba.Pix
	for y := range h {
		for x := range w {
			i := y*w + x
			pixelIdx := (y * rgba.Stride) + (x * 4)

			// Clamp and convert to uint8 in one step
			pix[pixelIdx+0] = uint8(math.Max(0, math.Min(255, float64(rPlane[i])*255+0.5))) // R
			pix[pixelIdx+1] = uint8(math.Max(0, math.Min(255, float64(gPlane[i])*255+0.5))) // G
			pix[pixelIdx+2] = uint8(math.Max(0, math.Min(255, float64(bPlane[i])*255+0.5))) // B
			pix[pixelIdx+3] = 255                                                           // A
		}
	}

	return rgba, nil
}
