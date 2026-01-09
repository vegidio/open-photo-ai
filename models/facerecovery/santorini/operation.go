package santorini

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFrSantorini struct {
	precision types.Precision
}

func (o OpFrSantorini) Id() string {
	return fmt.Sprintf("fr_santorini_%s", o.precision)
}

func (o OpFrSantorini) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpFrSantorini)(nil)

func Op(precision types.Precision) OpFrSantorini {
	return OpFrSantorini{
		precision: precision,
	}
}
