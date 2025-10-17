package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/types"
)

type OpUpscale struct {
	id    string
	scale int
	mode  Mode
}

func (o OpUpscale) Id() string {
	return o.id
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpUpscale)(nil)

func Op(scale int, mode Mode) OpUpscale {
	return OpUpscale{
		id:    fmt.Sprintf("upscale_%dx_%s", scale, mode),
		scale: scale,
		mode:  mode,
	}
}

// Mode is the type of upscale operation.
type Mode string

// Constants for supported image formats.
const (
	ModeCartoon Mode = "cartoon"
	ModeMedium  Mode = "medium"
	ModeHigh    Mode = "high"
)
