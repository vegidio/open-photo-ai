package kyoto

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpUpKyoto struct {
	id        string
	precision types.Precision
	mode      Mode
	scale     int
}

func (o OpUpKyoto) Id() string {
	return o.id
}

func (o OpUpKyoto) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpUpKyoto)(nil)

func Op(mode Mode, scale int, precision types.Precision) OpUpKyoto {
	return OpUpKyoto{
		id:        fmt.Sprintf("up_kyoto_%s_%dx_%s", mode, scale, precision),
		precision: precision,
		mode:      mode,
		scale:     scale,
	}
}

// Mode is the type of Kyoto upscale operation.
type Mode string

// Constants for the Kyoto upscale modes.
const (
	ModeGeneral Mode = "general"
	ModeCartoon Mode = "cartoon"
)
