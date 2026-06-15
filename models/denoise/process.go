package denoise

import (
	"context"
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// RunPipeline denoises an entire image using the given fixed-shape session, processing it in overlapping 256x256 tiles
// and stitching the results back together. The output preserves the input dimensions (denoise does not scale), so the
// shared tiled-inference driver runs with a scale factor of 1.
func RunPipeline(
	ctx context.Context,
	session *ort.DynamicAdvancedSession,
	img image.Image,
	onProgress types.InferenceProgress,
	opts ...utils.TileOption,
) (image.Image, error) {
	if onProgress != nil {
		onProgress("dn", 0)
	}

	return utils.RunTiledInference(ctx, session, img, 1, "dn", onProgress, opts...)
}
