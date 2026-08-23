package upscale

import (
	"context"
	"fmt"
	"image"
	"math"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// RunPipeline upscales an image by running each scale-factor session in turn over overlapping tiles (via the shared
// tiled-inference driver) and finally resizing to the intended scale.
func RunPipeline(
	ctx context.Context,
	sessions []*utils.Session,
	img image.Image,
	scales []int,
	intendedScale float64,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress(0)
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
			wrapped = func(p float64) { onProgress(base + p*frac) }
		}

		start := time.Now()

		processedImg, err := utils.RunTiledInference(ctx, session, resultImg, scales[i], wrapped)
		if err != nil {
			return nil, errors.Wrap(err, "failed to process image")
		}

		// A multi-pass upscale is the longest single wait the app ever asks a user to sit through, and the operation's
		// own duration says nothing about how it was divided. Debug, since it fires per pass rather than per operation.
		internal.Log().Debug("upscale pass complete", "pass", i+1, "of", len(sessions),
			"scale", scales[i], "width", resultImg.Bounds().Dx(), "height", resultImg.Bounds().Dy(),
			"duration", time.Since(start))

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

// MinScale and MaxScale bound what any upscale operation may be asked for. They live here, beside the scale matrices,
// because they are a property of the family rather than of any one model - a variant that disagreed would let the UI
// offer a scale its siblings reject.
const (
	MinScale = 1.0
	MaxScale = 8.0
)

// ClampScale bounds a requested scale to what the upscalers accept. Every variant's Op runs its argument through this,
// so an out-of-range request is corrected identically no matter which model receives it.
func ClampScale(scale float64) float64 {
	return min(max(scale, MinScale), MaxScale)
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

// ParamScale is the map key carrying the per-run scale factor to Model.Run. Only variants whose Id omits the scale
// (see Op) use it; for the rest the scale is in the Id and the params map is empty.
const ParamScale = "scale"

// ScaleFromParams reads the per-run scale from a params map, defaulting to 1.0 when absent or of the wrong type.
// A scale of 1.0 still runs a diffusion model: it restores detail at the input size.
func ScaleFromParams(params map[string]any) float64 {
	if v, ok := params[ParamScale].(float64); ok {
		return ClampScale(v)
	}

	return 1.0
}

// ScaleCacheKey is the stable per-run signature folded into the image cache key.
func ScaleCacheKey(scale float64) string {
	return fmt.Sprintf("s=%.4g", scale)
}
