package kyoto

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
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

func (o OpUpKyoto) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpUpKyoto)(nil)

func Op(scale float64, precision types.Precision) OpUpKyoto {
	if scale < 1 {
		scale = 1
	}
	if scale > 8 {
		scale = 8
	}

	return OpUpKyoto{
		precision: precision,
		scale:     scale,
	}
}
