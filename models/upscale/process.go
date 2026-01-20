package upscale

import (
	"context"
	"image"
	"math"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

const (
	tileOverlap = 16
	tileSize    = 256
)

func RunPipeline(
	ctx context.Context,
	sessions []*ort.DynamicAdvancedSession,
	input *types.ImageData,
	scales []int,
	intendedScale float64,
	onProgress types.InferenceProgress,
) (*types.ImageData, error) {
	if onProgress != nil {
		onProgress("up", 0)
	}

	img := input.Pixels

	for i, session := range sessions {
		result, err := process(ctx, session, img, scales[i], onProgress)
		if err != nil {
			return nil, err
		}

		img = result
	}

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   resizeToIntendedScale(img, input.Pixels.Bounds(), intendedScale),
	}, nil
}

// Process upscales an entire image by processing it in overlapping tiles
func process(
	ctx context.Context,
	session *ort.DynamicAdvancedSession,
	img image.Image,
	scaleFactor int,
	onProgress types.InferenceProgress,
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
	stride := tileSize - tileOverlap

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
			blendTileWithOverlap(result, upscaledTile, outputX, outputY, tileOverlap*scaleFactor, x > 0, y > 0)

			if onProgress != nil {
				total += step
				onProgress("up", utils.Ceiling(total))
			}
		}
	}

	return result, nil
}
