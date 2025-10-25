package upscale

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"math"

	ort "github.com/yalue/onnxruntime_go"
)

func extractTile(img image.Image, x1, y1, x2, y2, targetWidth, targetHeight int) image.Image {
	actualWidth := x2 - x1
	actualHeight := y2 - y1

	// Create tile with fixed target size
	tile := image.NewRGBA(image.Rect(0, 0, targetWidth, targetHeight))

	// Copy actual image data
	for y := 0; y < actualHeight; y++ {
		for x := 0; x < actualWidth; x++ {
			tile.Set(x, y, img.At(x1+x, y1+y))
		}
	}

	// Pad right edge if needed (replicate last column)
	if actualWidth < targetWidth {
		for y := 0; y < actualHeight; y++ {
			edgeColor := img.At(x2-1, y1+y)
			for x := actualWidth; x < targetWidth; x++ {
				tile.Set(x, y, edgeColor)
			}
		}
	}

	// Pad bottom edge if needed (replicate last row)
	if actualHeight < targetHeight {
		for x := 0; x < targetWidth; x++ {
			var edgeColor color.Color
			if x < actualWidth {
				edgeColor = img.At(x1+x, y2-1)
			} else {
				// Bottom-right corner: use the bottom-right pixel
				edgeColor = img.At(x2-1, y2-1)
			}
			for y := actualHeight; y < targetHeight; y++ {
				tile.Set(x, y, edgeColor)
			}
		}
	}

	return tile
}

func imageToNCHW(img image.Image) ([]float32, int, int) {
	rgba := image.NewRGBA(img.Bounds())
	draw.Draw(rgba, rgba.Bounds(), img, img.Bounds().Min, draw.Src)
	h, w := rgba.Bounds().Dy(), rgba.Bounds().Dx()
	data := make([]float32, 3*w*h)

	for y := 0; y < h; y++ {
		row := y * rgba.Stride
		for x := 0; x < w; x++ {
			i := row + 4*x
			idx := y*w + x
			data[0*w*h+idx] = float32(rgba.Pix[i+0]) / 255.0 // R
			data[1*w*h+idx] = float32(rgba.Pix[i+1]) / 255.0 // G
			data[2*w*h+idx] = float32(rgba.Pix[i+2]) / 255.0 // B
		}
	}

	return data, h, w
}

func tensorToRGBA(t *ort.Tensor[float32]) (*image.RGBA, error) {
	data := t.GetData()   // flat []float32
	shape := t.GetShape() // []int64, e.g. [1, 3, H, W]
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

	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			i := y*w + x
			r := uint8(math.Round(math.Max(0, math.Min(1, float64(rPlane[i]))) * 255))
			g := uint8(math.Round(math.Max(0, math.Min(1, float64(gPlane[i]))) * 255))
			b := uint8(math.Round(math.Max(0, math.Min(1, float64(bPlane[i]))) * 255))
			rgba.SetRGBA(x, y, color.RGBA{r, g, b, 255})
		}
	}

	return rgba, nil
}
