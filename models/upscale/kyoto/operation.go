package kyoto

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

type OpUpKyoto struct {
	precision types.Precision
	scale     float64
}

func (o OpUpKyoto) Id() string {
	return fmt.Sprintf("up_kyoto_%.4gx_%s", o.scale, o.precision)
}

func (o OpUpKyoto) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpUpKyoto)(nil)

func Op(scale float64, precision types.Precision) OpUpKyoto {
	return OpUpKyoto{
		precision: precision,
		scale:     upscale.ClampScale(scale),
	}
}
