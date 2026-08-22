package mumbai

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpClMumbai struct {
	precision types.Precision
}

func (o OpClMumbai) Id() string {
	return fmt.Sprintf("cl_mumbai_%s", o.precision)
}

func (o OpClMumbai) Precision() types.Precision {
	return o.precision
}

// Colorization has no per-run inputs, so OpClMumbai deliberately implements neither Parameterized nor CacheKeyer:
// the operation Id alone identifies both the model file and the cached result.
var _ types.Operation = (*OpClMumbai)(nil)

func Op(precision types.Precision) OpClMumbai {
	return OpClMumbai{precision: precision}
}
