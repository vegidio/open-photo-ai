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
	return fmt.Sprintf("la_paris_%.3g_%s", o.intensity, o.precision)
}

func (o OpLaParis) Precision() types.Precision {
	return o.precision
}

func (o OpLaParis) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpLaParis)(nil)

func Op(intensity float32, precision types.Precision) OpLaParis {
	return OpLaParis{
		intensity: intensity,
		precision: precision,
	}
}
