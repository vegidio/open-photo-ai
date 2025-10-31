package upscale

import (
	"fmt"
	"image"
	"image/draw"
	"math"

	ort "github.com/yalue/onnxruntime_go"
)

func extractTile(img image.Image, x1, y1, x2, y2, targetWidth, targetHeight int) image.Image {
	actualWidth := x2 - x1
	actualHeight := y2 - y1

	// Create tile with fixed target size
	tile := image.NewRGBA(image.Rect(0, 0, targetWidth, targetHeight))

	// Convert source to RGBA for direct pixel access
	var srcRGBA *image.RGBA
	if rgba, ok := img.(*image.RGBA); ok {
		srcRGBA = rgba
	} else {
		srcRGBA = image.NewRGBA(img.Bounds())
		draw.Draw(srcRGBA, srcRGBA.Bounds(), img, img.Bounds().Min, draw.Src)
	}

	srcPix := srcRGBA.Pix
	srcStride := srcRGBA.Stride
	dstPix := tile.Pix
	dstStride := tile.Stride

	// Copy actual image data (optimized row-by-row copy)
	for y := 0; y < actualHeight; y++ {
		srcRowStart := (y1+y)*srcStride + x1*4
		dstRowStart := y * dstStride
		copy(dstPix[dstRowStart:dstRowStart+actualWidth*4], srcPix[srcRowStart:srcRowStart+actualWidth*4])
	}

	// Pad right edge if needed (replicate last column)
	if actualWidth < targetWidth {
		for y := 0; y < actualHeight; y++ {
			// Get edge pixel from source
			edgeIdx := (y1+y)*srcStride + (x2-1)*4
			r, g, b, a := srcPix[edgeIdx], srcPix[edgeIdx+1], srcPix[edgeIdx+2], srcPix[edgeIdx+3]

			// Replicate across padding
			dstRowStart := y*dstStride + actualWidth*4
			for x := actualWidth; x < targetWidth; x++ {
				idx := dstRowStart + (x-actualWidth)*4
				dstPix[idx] = r
				dstPix[idx+1] = g
				dstPix[idx+2] = b
				dstPix[idx+3] = a
			}
		}
	}

	// Pad bottom edge if needed (replicate last row)
	if actualHeight < targetHeight {
		// First copy the last valid row for the actual width portion
		srcLastRow := (y2-1)*srcStride + x1*4
		for y := actualHeight; y < targetHeight; y++ {
			dstRowStart := y * dstStride
			copy(dstPix[dstRowStart:dstRowStart+actualWidth*4], srcPix[srcLastRow:srcLastRow+actualWidth*4])
		}

		// Then handle the bottom-right corner if there's right padding
		if actualWidth < targetWidth {
			cornerIdx := (y2-1)*srcStride + (x2-1)*4
			r, g, b, a := srcPix[cornerIdx], srcPix[cornerIdx+1], srcPix[cornerIdx+2], srcPix[cornerIdx+3]

			for y := actualHeight; y < targetHeight; y++ {
				dstRowStart := y*dstStride + actualWidth*4
				for x := actualWidth; x < targetWidth; x++ {
					idx := dstRowStart + (x-actualWidth)*4
					dstPix[idx] = r
					dstPix[idx+1] = g
					dstPix[idx+2] = b
					dstPix[idx+3] = a
				}
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

			// Sequential writes for better cache locality
			data[idx] = r
			data[planeSize+idx] = g
			data[2*planeSize+idx] = b
		}
	}

	return data, h, w
}

func tensorToRGBA(t *ort.Tensor[float32]) (*image.RGBA, error) {
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
