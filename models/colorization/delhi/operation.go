package delhi

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpClDelhi struct {
	precision types.Precision
}

func (o OpClDelhi) Id() string {
	return fmt.Sprintf("cl_delhi_%s", o.precision)
}

func (o OpClDelhi) Precision() types.Precision {
	return o.precision
}

// Colorization has no per-run inputs, so OpClDelhi deliberately implements neither Parameterized nor CacheKeyer:
// the operation Id alone identifies both the model file and the cached result.
var _ types.Operation = (*OpClDelhi)(nil)

func Op(precision types.Precision) OpClDelhi {
	return OpClDelhi{precision: precision}
}
