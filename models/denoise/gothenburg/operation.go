package gothenburg

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnGothenburg struct {
	intensity float32
	precision types.Precision
}

func (o OpDnGothenburg) Id() string {
	return fmt.Sprintf("dn_gothenburg_%s", o.precision)
}

func (o OpDnGothenburg) Precision() types.Precision {
	return o.precision
}

func (o OpDnGothenburg) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run denoise intensity, which is not part of the operation identity.
func (o OpDnGothenburg) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpDnGothenburg) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpDnGothenburg)(nil)
	_ types.Parameterized = (*OpDnGothenburg)(nil)
	_ types.CacheKeyer    = (*OpDnGothenburg)(nil)
)

func Op(intensity float32, precision types.Precision) OpDnGothenburg {
	return OpDnGothenburg{
		intensity: intensity,
		precision: precision,
	}
}
