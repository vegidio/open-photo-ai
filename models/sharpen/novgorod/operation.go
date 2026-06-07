package novgorod

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpShNovgorod struct {
	intensity float32
	precision types.Precision
}

func (o OpShNovgorod) Id() string {
	return fmt.Sprintf("sh_novgorod_%s", o.precision)
}

func (o OpShNovgorod) Precision() types.Precision {
	return o.precision
}

func (o OpShNovgorod) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run sharpen intensity, which is not part of the operation identity.
func (o OpShNovgorod) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpShNovgorod) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpShNovgorod)(nil)
	_ types.Parameterized = (*OpShNovgorod)(nil)
	_ types.CacheKeyer    = (*OpShNovgorod)(nil)
)

func Op(intensity float32, precision types.Precision) OpShNovgorod {
	return OpShNovgorod{
		intensity: intensity,
		precision: precision,
	}
}
