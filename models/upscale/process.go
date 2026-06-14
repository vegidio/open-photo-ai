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

	// Weight each pass by its input pixel area (≈ tile count ≈ work) so progress advances monotonically
	// across multiple scale passes instead of resetting to 0 at the start of each one.
	weights := make([]float64, len(sessions))
	var totalW float64
	w, h := img.Bounds().Dx(), img.Bounds().Dy()
	for i := range sessions {
		weights[i] = float64(w * h)
		totalW += weights[i]
		w *= scales[i]
		h *= scales[i]
	}

	resultImg := img
	var done float64

	for i, session := range sessions {
		wrapped := onProgress
		if onProgress != nil && totalW > 0 {
			base, frac := done/totalW, weights[i]/totalW
			wrapped = func(op string, p float64) { onProgress(op, base+p*frac) }
		}

		processedImg, err := utils.RunTiledInference(ctx, session, resultImg, scales[i], "up", wrapped)
		if err != nil {
			return nil, errors.Wrap(err, "failed to process image")
		}

		resultImg = processedImg
		done += weights[i]
	}

	return resizeToIntendedScale(resultImg, img.Bounds(), intendedScale), nil
}

// ScaleBucket maps an upper scale bound to the sequence of native scale-factor passes that cover it.
type ScaleBucket struct {
	Max    float64
	Passes []int
}

// DefaultScaleBuckets is shared by variants with only a native 4x model (tokyo, saitama).
var DefaultScaleBuckets = []ScaleBucket{
	{Max: 4, Passes: []int{4}},
	{Max: 8, Passes: []int{4, 4}},
}

// SelectScaleMatrix returns the passes for the first bucket whose Max covers scale, or nil if none match.
func SelectScaleMatrix(scale float64, buckets []ScaleBucket) []int {
	for _, b := range buckets {
		if scale <= b.Max {
			return b.Passes
		}
	}

	return nil
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
