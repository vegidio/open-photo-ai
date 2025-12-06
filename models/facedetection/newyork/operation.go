package newyork

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFdNewYork struct {
	id        string
	precision types.Precision
}

func (o OpFdNewYork) Id() string {
	return o.id
}

func (o OpFdNewYork) Precision() types.Precision {
	return o.precision
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpFdNewYork)(nil)

func Op(precision types.Precision) OpFdNewYork {
	return OpFdNewYork{
		id:        fmt.Sprintf("fd_newyork_%s", precision),
		precision: precision,
	}
}
