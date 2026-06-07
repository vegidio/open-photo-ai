package petersburg

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpShPetersburg struct {
	intensity float32
	precision types.Precision
}

func (o OpShPetersburg) Id() string {
	return fmt.Sprintf("sh_petersburg_%s", o.precision)
}

func (o OpShPetersburg) Precision() types.Precision {
	return o.precision
}

func (o OpShPetersburg) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run sharpen intensity, which is not part of the operation identity.
func (o OpShPetersburg) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpShPetersburg) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpShPetersburg)(nil)
	_ types.Parameterized = (*OpShPetersburg)(nil)
	_ types.CacheKeyer    = (*OpShPetersburg)(nil)
)

func Op(intensity float32, precision types.Precision) OpShPetersburg {
	return OpShPetersburg{
		intensity: intensity,
		precision: precision,
	}
}
