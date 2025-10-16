package upscale

import "github.com/vegidio/open-photo-ai/internal/types"

type OpUpscale struct {
	scale int
}

func (m OpUpscale) IsOperation() {}

// Compile-time assertion to ensure it conforms to the Operation interface.
var _ types.Operation = (*OpUpscale)(nil)

func NewOperation() *OpUpscale {
	return &OpUpscale{
		scale: 4,
	}
}
