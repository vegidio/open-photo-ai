package saitama

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
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

func (o OpUpSaitama) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpUpSaitama)(nil)

func Op(scale float64, precision types.Precision) OpUpSaitama {
	if scale < 1 {
		scale = 1
	}
	if scale > 8 {
		scale = 8
	}

	return OpUpSaitama{
		precision: precision,
		scale:     scale,
	}
}
