package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpUpscale struct {
	id        string
	precision types.Precision
	scale     int
	mode      Mode
}

func (o OpUpscale) Id() string {
	return o.id
}

func (o OpUpscale) Precision() types.Precision {
	return o.precision
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpUpscale)(nil)

func Op(scale int, mode Mode, precision types.Precision) OpUpscale {
	return OpUpscale{
		id:        fmt.Sprintf("upscale_%dx_%s_%s", scale, mode, precision),
		scale:     scale,
		mode:      mode,
		precision: precision,
	}
}

// Mode is the type of upscale operation.
type Mode string

// Constants for supported image formats.
const (
	ModeGeneral Mode = "general"
	ModeCartoon Mode = "cartoon"
)
