package utils

import (
	"context"
	"image"
	"image/draw"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

const (
	tileOverlap = 16
	tileSize    = 256
)

// RunTiledInference runs a fixed-shape ONNX session over an image in overlapping 256x256 tiles and stitches the results
// back together with soft blending. scale is the model's output scale factor (1 for denoise, N for an NxN upscale), so
// the result has dimensions width*scale x height*scale. opId is the progress key (e.g. "dn"/"up").
//
// The caller is responsible for emitting the initial onProgress(opId, 0): upscale invokes this once per scale pass and
// must not reset progress to 0 on each pass.
func RunTiledInference(
	ctx context.Context,
	session *ort.DynamicAdvancedSession,
	img image.Image,
	scale int,
	opId string,
	onProgress types.InferenceProgress,
) (*image.RGBA, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Get image dimensions
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	// Create output image (scaled)
	result := image.NewRGBA(image.Rect(0, 0, width*scale, height*scale))

	// Calculate tile stride (step size)
	stride := tileSize - tileOverlap

	step := 1 / (math.Ceil(float64(height)/float64(stride)) * math.Ceil(float64(width)/float64(stride)))
	total := 0.0

	// Process image in tiles
	for y := 0; y < height; y += stride {
		for x := 0; x < width; x += stride {
			if err := ctx.Err(); err != nil {
				return nil, errors.Wrap(err, "context cancelled")
			}

			tileX, tileY, tileW, tileH := calculateTileBounds(x, y, width, height, tileSize)

			paddedTile := prepareTileForInference(img, tileX, tileY, tileW, tileH, tileSize)

			processedTile, err := processTile(session, paddedTile, tileW, tileH, scale)
			if err != nil {
				return nil, errors.Wrap(err, "failed to process tile")
			}

			outputX := tileX * scale
			outputY := tileY * scale
			blendTileWithOverlap(result, processedTile, outputX, outputY, tileOverlap*scale, x > 0, y > 0)

			if onProgress != nil {
				total += step
				onProgress(opId, ClampProgress(total))
			}
		}
	}

	return result, nil
}

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

// processTile runs the ML model inference and removes padding from the result. The crop bounds are scaled by scale to
// match the inference output dimensions (scale is 1 for denoise).
func processTile(session *ort.DynamicAdvancedSession, tile image.Image, tileW, tileH, scale int) (image.Image, error) {
	// Run inference
	processedTile, err := runTileInference(session, tile, scale)
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	// Remove padding by cropping to the actual content size (scaled)
	cropW := tileW * scale
	cropH := tileH * scale
	croppedTile := imaging.Crop(processedTile, image.Rect(0, 0, cropW, cropH))

	return croppedTile, nil
}

// runTileInference runs inference on a single padded tile. The output shares the input's shape scaled by scale (scale 1
// keeps the dimensions, e.g. denoise; scale N upscales).
func runTileInference(session *ort.DynamicAdvancedSession, tile image.Image, scale int) (image.Image, error) {
	bounds := tile.Bounds()
	h, w := bounds.Dy(), bounds.Dx()

	inputData := ImageToCHW(tile, true, false)

	inputShape := ort.NewShape(1, 3, int64(h), int64(w))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	outputShape := ort.NewShape(1, 3, int64(h*scale), int64(w*scale))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create output tensor")
	}
	defer outputTensor.Destroy()

	err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	outputData := outputTensor.GetData()
	outW := int(outputShape[3])
	outH := int(outputShape[2])

	return CHWToImage(outputData, outW, outH, false), nil
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
	draw.Draw(padded, image.Rect(left, top, left+width, top+height), img, bounds.Min, draw.Src)

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
