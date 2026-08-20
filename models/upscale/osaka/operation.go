package osaka

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// ParamScale is the map key carrying the per-run scale factor to Model.Run.
const ParamScale = "scale"

// OpUpOsaka is the SeedVR2-backed diffusion upscaler.
//
// Unlike the other upscale operations, the scale is NOT part of the identity. SeedVR2 does not change resolution: the
// image is resampled to the target size first and the model restores detail at that size, so one set of sessions
// serves every scale. Since those sessions are over 7 GB, putting the scale in the Id would turn a scale change into
// a registry miss and a multi-gigabyte rebuild. It travels in Params instead, exactly as the denoise intensity does.
type OpUpOsaka struct {
	precision types.Precision
	scale     float64
}

func (o OpUpOsaka) Id() string {
	return fmt.Sprintf("up_osaka_%s", o.precision)
}

func (o OpUpOsaka) Precision() types.Precision {
	return o.precision
}

// Params carries the per-run scale factor, which is not part of the operation identity.
func (o OpUpOsaka) Params() map[string]any {
	return map[string]any{ParamScale: o.scale}
}

// CacheKey folds the scale into the image cache key so different scales don't collide on the shared Id.
func (o OpUpOsaka) CacheKey() string {
	return ScaleCacheKey(o.scale)
}

var (
	_ types.Operation     = (*OpUpOsaka)(nil)
	_ types.Parameterized = (*OpUpOsaka)(nil)
	_ types.CacheKeyer    = (*OpUpOsaka)(nil)
)

// Op builds the operation for a scale factor.
//
// The precision argument is accepted for symmetry with the other upscalers but ignored: only an fp16 build of this
// model is published. Honouring an fp32 request would produce an Id with no entry in the remote manifest, which the
// dependency installer treats as a model to fetch unverified - so a typo downstream would become a 404 downloaded
// without a hash check rather than a clear failure.
func Op(scale float64, _ types.Precision) OpUpOsaka {
	return OpUpOsaka{
		precision: types.PrecisionFp16,
		scale:     upscale.ClampScale(scale),
	}
}

// ScaleFromParams reads the per-run scale from a params map, defaulting to 1.0 when absent or of the wrong type.
// A scale of 1.0 still runs the model: SeedVR2 restores detail at the input size.
func ScaleFromParams(params map[string]any) float64 {
	if v, ok := params[ParamScale].(float64); ok {
		return upscale.ClampScale(v)
	}

	return 1.0
}

// ScaleCacheKey is the stable per-run signature folded into the image cache key.
func ScaleCacheKey(scale float64) string {
	return fmt.Sprintf("s=%.4g", scale)
}
