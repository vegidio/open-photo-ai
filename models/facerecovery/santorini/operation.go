package santorini

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFrSantorini struct {
	id        string
	precision types.Precision
}

func (o OpFrSantorini) Id() string {
	return o.id
}

func (o OpFrSantorini) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpFrSantorini)(nil)

func Op(precision types.Precision) OpFrSantorini {
	return OpFrSantorini{
		id:        fmt.Sprintf("fr_santorini_%s", precision),
		precision: precision,
	}
}
