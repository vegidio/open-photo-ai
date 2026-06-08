package malmo

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnMalmo struct {
	intensity float32
	precision types.Precision
}

func (o OpDnMalmo) Id() string {
	return fmt.Sprintf("dn_malmo_%s", o.precision)
}

func (o OpDnMalmo) Precision() types.Precision {
	return o.precision
}

func (o OpDnMalmo) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run denoise intensity, which is not part of the operation identity.
func (o OpDnMalmo) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpDnMalmo) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpDnMalmo)(nil)
	_ types.Parameterized = (*OpDnMalmo)(nil)
	_ types.CacheKeyer    = (*OpDnMalmo)(nil)
)

func Op(intensity float32, precision types.Precision) OpDnMalmo {
	return OpDnMalmo{
		intensity: intensity,
		precision: precision,
	}
}
