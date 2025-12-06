package facedetection

import (
	"image"

	"github.com/disintegration/imaging"
)

// PreprocessImage handles image resizing and tensor data preparation
func PreprocessImage(img image.Image, targetSize int) ([]float32, float32, float32) {
	bounds := img.Bounds()
	originalWidth := float32(bounds.Dx())
	originalHeight := float32(bounds.Dy())

	// Calculate resize dimensions maintaining aspect ratio
	newWidth, newHeight := calculateResizeDimensions(originalWidth, originalHeight, targetSize)

	// Resize image
	resized := imaging.Resize(img, newWidth, newHeight, imaging.Lanczos)

	// Create padded input tensor data with mean subtraction
	inputData := createInputTensorData(resized, newWidth, newHeight, targetSize)

	return inputData, originalWidth, originalHeight
}

// calculateResizeDimensions calculates new dimensions maintaining aspect ratio
func calculateResizeDimensions(width, height float32, targetSize int) (int, int) {
	imRatio := height / width
	var newWidth, newHeight int

	if imRatio > 1.0 {
		newHeight = targetSize
		newWidth = int(float32(newHeight) / imRatio)
	} else {
		newWidth = targetSize
		newHeight = int(float32(newWidth) * imRatio)
	}

	return newWidth, newHeight
}

// createInputTensorData creates padded input data with mean subtraction (BGR format, CHW layout)
func createInputTensorData(resized image.Image, newWidth, newHeight, targetSize int) []float32 {
	const (
		meanB = float32(104.0)
		meanG = float32(117.0)
		meanR = float32(123.0)
	)

	inputData := make([]float32, 3*targetSize*targetSize)

	channelSize := targetSize * targetSize
	bOffset := 0
	gOffset := channelSize
	rOffset := 2 * channelSize

	// Process the actual image area
	for y := 0; y < newHeight; y++ {
		rowOffset := y * targetSize

		for x := 0; x < newWidth; x++ {
			r, g, b, _ := resized.At(x, y).RGBA()
			idx := rowOffset + x

			// Convert from 16-bit to 8-bit and subtract mean
			inputData[bOffset+idx] = float32(b>>8) - meanB
			inputData[gOffset+idx] = float32(g>>8) - meanG
			inputData[rOffset+idx] = float32(r>>8) - meanR
		}

		// Fill the padding area in this row (if any) with negative mean values
		for x := newWidth; x < targetSize; x++ {
			idx := rowOffset + x
			inputData[bOffset+idx] = -meanB
			inputData[gOffset+idx] = -meanG
			inputData[rOffset+idx] = -meanR
		}
	}

	// Fill the remaining rows with padding (negative mean values)
	for y := newHeight; y < targetSize; y++ {
		rowOffset := y * targetSize

		for x := 0; x < targetSize; x++ {
			idx := rowOffset + x
			inputData[bOffset+idx] = -meanB
			inputData[gOffset+idx] = -meanG
			inputData[rOffset+idx] = -meanR
		}
	}

	return inputData
}
