package rio

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpCbRio struct {
	intensity float32
	precision types.Precision
}

func (o OpCbRio) Id() string {
	return fmt.Sprintf("cb_rio_%s", o.precision)
}

func (o OpCbRio) Precision() types.Precision {
	return o.precision
}

func (o OpCbRio) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run blend intensity, which is not part of the operation identity.
func (o OpCbRio) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpCbRio) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpCbRio)(nil)
	_ types.Parameterized = (*OpCbRio)(nil)
	_ types.CacheKeyer    = (*OpCbRio)(nil)
)

func Op(intensity float32, precision types.Precision) OpCbRio {
	return OpCbRio{
		intensity: intensity,
		precision: precision,
	}
}
