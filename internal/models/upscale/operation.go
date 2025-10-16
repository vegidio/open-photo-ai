package upscale

import "github.com/vegidio/open-photo-ai/internal/types"

type OpUpscale struct {
	id    string
	scale int
}

func (o OpUpscale) Id() string {
	return o.id
}

// Compile-time assertion to ensure it conforms to the Operation interface.
var _ types.Operation = (*OpUpscale)(nil)

func Operation() *OpUpscale {
	return &OpUpscale{
		id:    "upscale",
		scale: 4,
	}
}
