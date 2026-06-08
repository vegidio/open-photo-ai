package stockholm

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnStockholm struct {
	intensity float32
	precision types.Precision
}

func (o OpDnStockholm) Id() string {
	return fmt.Sprintf("dn_stockholm_%s", o.precision)
}

func (o OpDnStockholm) Precision() types.Precision {
	return o.precision
}

func (o OpDnStockholm) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params carries the per-run denoise intensity, which is not part of the operation identity.
func (o OpDnStockholm) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o OpDnStockholm) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*OpDnStockholm)(nil)
	_ types.Parameterized = (*OpDnStockholm)(nil)
	_ types.CacheKeyer    = (*OpDnStockholm)(nil)
)

func Op(intensity float32, precision types.Precision) OpDnStockholm {
	return OpDnStockholm{
		intensity: intensity,
		precision: precision,
	}
}
