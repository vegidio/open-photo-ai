package jaipur

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpClJaipur struct {
	precision types.Precision
}

func (o OpClJaipur) Id() string {
	return fmt.Sprintf("cl_jaipur_%s", o.precision)
}

func (o OpClJaipur) Precision() types.Precision {
	return o.precision
}

// Colorization has no per-run inputs, so OpClJaipur deliberately implements neither Parameterized nor CacheKeyer:
// the operation Id alone identifies both the model file and the cached result.
var _ types.Operation = (*OpClJaipur)(nil)

func Op(precision types.Precision) OpClJaipur {
	return OpClJaipur{precision: precision}
}
