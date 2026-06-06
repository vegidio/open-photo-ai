package newyork

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDtNewYork struct {
	precision types.Precision
}

func (o OpDtNewYork) Id() string {
	return fmt.Sprintf("dt_newyork_%s", o.precision)
}

func (o OpDtNewYork) Precision() types.Precision {
	return o.precision
}

func (o OpDtNewYork) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpDtNewYork)(nil)

func Op(precision types.Precision) OpDtNewYork {
	return OpDtNewYork{
		precision: precision,
	}
}
