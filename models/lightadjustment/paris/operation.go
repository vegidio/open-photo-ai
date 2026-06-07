package paris

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpLaParis struct {
	intensity float32
	precision types.Precision
}

func (o OpLaParis) Id() string {
	return fmt.Sprintf("la_paris_%s", o.precision)
}

func (o OpLaParis) Precision() types.Precision {
	return o.precision
}

func (o OpLaParis) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run blend intensity, which is not part of the operation identity.
func (o OpLaParis) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpLaParis) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpLaParis)(nil)
	_ types.Parameterized = (*OpLaParis)(nil)
	_ types.CacheKeyer    = (*OpLaParis)(nil)
)

func Op(intensity float32, precision types.Precision) OpLaParis {
	return OpLaParis{
		intensity: intensity,
		precision: precision,
	}
}
