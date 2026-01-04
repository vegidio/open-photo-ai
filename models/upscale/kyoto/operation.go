package kyoto

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpUpKyoto struct {
	precision types.Precision
	mode      Mode
	scale     float64
}

func (o OpUpKyoto) Id() string {
	return fmt.Sprintf("up_kyoto_%s_%.4gx_%s", o.mode, o.scale, o.precision)
}

func (o OpUpKyoto) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*OpUpKyoto)(nil)

func Op(mode Mode, scale float64, precision types.Precision) OpUpKyoto {
	if scale < 1 {
		scale = 1
	}
	if scale > 16 {
		scale = 16
	}

	return OpUpKyoto{
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
