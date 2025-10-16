package upscale

import "github.com/vegidio/open-photo-ai/internal/types"

type OpUpscale struct {
	id    string
	scale int
	mode  string
}

func (o OpUpscale) Id() string {
	return o.id
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpUpscale)(nil)

func Op(scale int, mode string) OpUpscale {
	return OpUpscale{
		id:    "upscale",
		scale: scale,
		mode:  mode,
	}
}
