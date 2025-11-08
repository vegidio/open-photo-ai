package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpUpscale struct {
	id        string
	mode      Mode
	scale     int
	precision types.Precision
}

func (o OpUpscale) Id() string {
	return o.id
}

func (o OpUpscale) Precision() types.Precision {
	return o.precision
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpUpscale)(nil)

func Op(mode Mode, scale int, precision types.Precision) OpUpscale {
	return OpUpscale{
		id:        fmt.Sprintf("upscale_%s_%dx_%s", mode, scale, precision),
		mode:      mode,
		scale:     scale,
		precision: precision,
	}
}

// Mode is the type of upscale operation.
type Mode string

// Constants for the upscale modes.
const (
	ModeGeneral Mode = "general"
	ModeCartoon Mode = "cartoon"
)
