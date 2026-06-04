package denoise

import (
	"context"
	"image"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

const (
	tileOverlap = 16
	tileSize    = 256
)

// RunPipeline denoises an entire image using the given fixed-shape session, processing it in overlapping 256x256 tiles
// and stitching the results back together. The output preserves the input dimensions (denoise does not scale).
func RunPipeline(
	ctx context.Context,
	session *ort.DynamicAdvancedSession,
	img image.Image,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress("dn", 0)
	}

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Get image dimensions
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	// Create output image (same dimensions as input)
	result := image.NewRGBA(image.Rect(0, 0, width, height))

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

			denoisedTile, err := denoiseTile(session, paddedTile, tileW, tileH)
			if err != nil {
				return nil, errors.Wrap(err, "failed to denoise tile")
			}

			blendTileWithOverlap(result, denoisedTile, tileX, tileY, tileOverlap, x > 0, y > 0)

			if onProgress != nil {
				total += step
				onProgress("dn", utils.Ceiling(total))
			}
		}
	}

	return result, nil
}
