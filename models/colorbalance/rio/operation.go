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
	return fmt.Sprintf("cb_rio_%.3g_%s", o.intensity, o.precision)
}

func (o OpCbRio) Precision() types.Precision {
	return o.precision
}

func (o OpCbRio) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpCbRio)(nil)

func Op(intensity float32, precision types.Precision) OpCbRio {
	return OpCbRio{
		intensity: intensity,
		precision: precision,
	}
}
