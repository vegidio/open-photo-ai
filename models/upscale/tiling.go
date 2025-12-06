package upscale

import (
	"image"

	"github.com/disintegration/imaging"
	ort "github.com/yalue/onnxruntime_go"
)

// calculateTileBounds adjusts tile coordinates to fit within image boundaries
func calculateTileBounds(x, y, imgWidth, imgHeight, tileSize int) (tileX, tileY, tileW, tileH int) {
	tileX = x
	tileY = y
	tileW = tileSize
	tileH = tileSize

	// Adjust horizontal bounds
	if tileX+tileW > imgWidth {
		tileX = imgWidth - tileW
		if tileX < 0 {
			tileX = 0
			tileW = imgWidth
		}
	}

	// Adjust vertical bounds
	if tileY+tileH > imgHeight {
		tileY = imgHeight - tileH
		if tileY < 0 {
			tileY = 0
			tileH = imgHeight
		}
	}

	return
}

// prepareTileForInference extracts a tile and pads it to the required size
func prepareTileForInference(img image.Image, tileX, tileY, tileW, tileH, tileSize int) image.Image {
	// Extract tile from the image
	tile := imaging.Crop(img, image.Rect(tileX, tileY, tileX+tileW, tileY+tileH))

	// Calculate required padding
	padRight := 0
	padBottom := 0

	if tileW < tileSize {
		padRight = tileSize - tileW
	}
	if tileH < tileSize {
		padBottom = tileSize - tileH
	}

	// Apply reflection padding if needed
	if padRight > 0 || padBottom > 0 {
		return reflectionPad(tile, 0, 0, padRight, padBottom)
	}

	return tile
}

// upscaleTile runs the ML model inference and removes padding from the result
func upscaleTile(session *ort.DynamicAdvancedSession, tile image.Image, tileW, tileH, scaleFactor int) (image.Image, error) {
	// Run inference
	upscaledTile, err := runInference(session, tile, scaleFactor)
	if err != nil {
		return nil, err
	}

	// Remove padding by cropping to the actual content size
	cropW := tileW * scaleFactor
	cropH := tileH * scaleFactor
	croppedTile := imaging.Crop(upscaledTile, image.Rect(0, 0, cropW, cropH))

	return croppedTile, nil
}

// reflectionPad adds reflection padding to an image
func reflectionPad(img image.Image, left, top, right, bottom int) image.Image {
	bounds := img.Bounds()
	width, height := bounds.Dx(), bounds.Dy()

	paddedWidth := width + left + right
	paddedHeight := height + top + bottom
	padded := image.NewRGBA(image.Rect(0, 0, paddedWidth, paddedHeight))

	// Helper to get reflected coordinate
	reflectIndex := func(idx, max int) int {
		if idx < 0 {
			return -idx - 1
		}
		if idx >= max {
			return 2*max - idx - 1
		}
		return idx
	}

	// Copy original image to center
	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			padded.Set(x+left, y+top, img.At(bounds.Min.X+x, bounds.Min.Y+y))
		}
	}

	// Pad left and right edges
	for y := 0; y < height; y++ {
		srcY := bounds.Min.Y + y
		dstY := y + top

		// Left padding
		for x := 0; x < left; x++ {
			srcX := bounds.Min.X + reflectIndex(left-x-1, width)
			padded.Set(x, dstY, img.At(srcX, srcY))
		}

		// Right padding
		for x := 0; x < right; x++ {
			srcX := bounds.Min.X + reflectIndex(width+x, width)
			padded.Set(width+left+x, dstY, img.At(srcX, srcY))
		}
	}

	// Pad top and bottom edges (including corners)
	for x := 0; x < paddedWidth; x++ {
		// Top padding
		for y := 0; y < top; y++ {
			srcY := reflectIndex(top-y-1, height) + top
			padded.Set(x, y, padded.At(x, srcY))
		}

		// Bottom padding
		for y := 0; y < bottom; y++ {
			srcY := reflectIndex(height+y, height) + top
			padded.Set(x, height+top+y, padded.At(x, srcY))
		}
	}

	return padded
}
