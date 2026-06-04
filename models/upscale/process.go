package upscale

import (
	"context"
	"image"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// RunPipeline upscales an image by running each scale-factor session in turn over overlapping tiles (via the shared
// tiled-inference driver) and finally resizing to the intended scale.
func RunPipeline(
	ctx context.Context,
	sessions []*ort.DynamicAdvancedSession,
	img image.Image,
	scales []int,
	intendedScale float64,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress("up", 0)
	}

	resultImg := img

	for i, session := range sessions {
		processedImg, err := utils.RunTiledInference(ctx, session, resultImg, scales[i], "up", onProgress)
		if err != nil {
			return nil, errors.Wrap(err, "failed to process image")
		}

		resultImg = processedImg
	}

	return resizeToIntendedScale(resultImg, img.Bounds(), intendedScale), nil
}

// resizeToIntendedScale rescales the given image to the specified scale factor while preserving its aspect ratio.
func resizeToIntendedScale(img image.Image, originalBounds image.Rectangle, scale float64) image.Image {
	width := int(math.Round(float64(originalBounds.Dx()) * scale))
	height := int(math.Round(float64(originalBounds.Dy()) * scale))

	if img.Bounds().Dx() != width || img.Bounds().Dy() != height {
		img = imaging.Resize(img, width, height, imaging.Lanczos)
	}

	return img
}
