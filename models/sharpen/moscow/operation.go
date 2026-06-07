package moscow

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpShMoscow struct {
	intensity float32
	precision types.Precision
}

func (o OpShMoscow) Id() string {
	return fmt.Sprintf("sh_moscow_%s", o.precision)
}

func (o OpShMoscow) Precision() types.Precision {
	return o.precision
}

func (o OpShMoscow) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run sharpen intensity, which is not part of the operation identity.
func (o OpShMoscow) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpShMoscow) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpShMoscow)(nil)
	_ types.Parameterized = (*OpShMoscow)(nil)
	_ types.CacheKeyer    = (*OpShMoscow)(nil)
)

func Op(intensity float32, precision types.Precision) OpShMoscow {
	return OpShMoscow{
		intensity: intensity,
		precision: precision,
	}
}
