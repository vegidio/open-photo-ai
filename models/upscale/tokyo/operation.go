package tokyo

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

type OpUpTokyo struct {
	precision types.Precision
	scale     float64
}

func (o OpUpTokyo) Id() string {
	return fmt.Sprintf("up_tokyo_%.4gx_%s", o.scale, o.precision)
}

func (o OpUpTokyo) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpUpTokyo)(nil)

func Op(scale float64, precision types.Precision) OpUpTokyo {
	return OpUpTokyo{
		precision: precision,
		scale:     upscale.ClampScale(scale),
	}
}
