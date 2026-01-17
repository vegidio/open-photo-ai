package newyork

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpFdNewYork struct {
	precision types.Precision
}

func (o OpFdNewYork) Id() string {
	return fmt.Sprintf("fd_newyork_%s", o.precision)
}

func (o OpFdNewYork) Precision() types.Precision {
	return o.precision
}

func (o OpFdNewYork) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpFdNewYork)(nil)

func Op(precision types.Precision) OpFdNewYork {
	return OpFdNewYork{
		precision: precision,
	}
}
