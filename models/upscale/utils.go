package upscale

import (
	"image"
	"image/draw"
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
