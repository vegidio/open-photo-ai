package saitama

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

type OpUpSaitama struct {
	precision types.Precision
	scale     float64
}

func (o OpUpSaitama) Id() string {
	return fmt.Sprintf("up_saitama_%.4gx_%s", o.scale, o.precision)
}

func (o OpUpSaitama) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpUpSaitama)(nil)

func Op(scale float64, precision types.Precision) OpUpSaitama {
	return OpUpSaitama{
		precision: precision,
		scale:     upscale.ClampScale(scale),
	}
}
