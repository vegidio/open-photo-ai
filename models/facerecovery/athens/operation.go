package athens

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFrAthens struct {
	precision types.Precision
}

func (o OpFrAthens) Id() string {
	return fmt.Sprintf("fr_athens_%s", o.precision)
}

func (o OpFrAthens) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpFrAthens)(nil)

func Op(precision types.Precision) OpFrAthens {
	return OpFrAthens{
		precision: precision,
	}
}
