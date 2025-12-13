package upscale

import (
	"context"
	"image"
	"math"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// Process upscales an entire image by processing it in overlapping tiles
func Process(
	ctx context.Context,
	session *ort.DynamicAdvancedSession,
	img image.Image,
	tileSize,
	overlap,
	scaleFactor int,
	onProgress types.ProgressCallback,
) (*image.RGBA, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	// Get image dimensions
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	// Create output image
	outputWidth := width * scaleFactor
	outputHeight := height * scaleFactor
	result := image.NewRGBA(image.Rect(0, 0, outputWidth, outputHeight))

	// Calculate tile stride (step size)
	stride := tileSize - overlap

	step := 1 / (math.Ceil(float64(height)/float64(stride)) * math.Ceil(float64(width)/float64(stride)))
	total := 0.0

	// Process image in tiles
	for y := 0; y < height; y += stride {
		for x := 0; x < width; x += stride {
			if err := ctx.Err(); err != nil {
				return nil, err
			}

			tileX, tileY, tileW, tileH := calculateTileBounds(x, y, width, height, tileSize)

			paddedTile := prepareTileForInference(img, tileX, tileY, tileW, tileH, tileSize)

			upscaledTile, err := upscaleTile(session, paddedTile, tileW, tileH, scaleFactor)
			if err != nil {
				return nil, err
			}

			outputX := tileX * scaleFactor
			outputY := tileY * scaleFactor
			blendTileWithOverlap(result, upscaledTile, outputX, outputY, overlap*scaleFactor, x > 0, y > 0)

			if onProgress != nil {
				total += step
				onProgress("up", utils.Ceiling(total))
			}
		}
	}

	return result, nil
}
