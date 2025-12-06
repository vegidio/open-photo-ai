package athens

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFrAthens struct {
	id        string
	precision types.Precision
}

func (o OpFrAthens) Id() string {
	return o.id
}

func (o OpFrAthens) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpFrAthens)(nil)

func Op(precision types.Precision) OpFrAthens {
	return OpFrAthens{
		id:        fmt.Sprintf("fr_athens_%s", precision),
		precision: precision,
	}
}
