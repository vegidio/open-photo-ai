package tokyo

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpUpTokyo struct {
	id        string
	precision types.Precision
	scale     int
}

func (o OpUpTokyo) Id() string {
	return o.id
}

func (o OpUpTokyo) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpUpTokyo)(nil)

func Op(scale int, precision types.Precision) OpUpTokyo {
	return OpUpTokyo{
		id:        fmt.Sprintf("up_tokyo_%dx_%s", scale, precision),
		precision: precision,
		scale:     scale,
	}
}
