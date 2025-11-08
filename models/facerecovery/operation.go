package facerecovery

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFaceRecovery struct {
	id        string
	mode      Mode
	precision types.Precision
}

func (o OpFaceRecovery) Id() string {
	return o.id
}

func (o OpFaceRecovery) Precision() types.Precision {
	return o.precision
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpFaceRecovery)(nil)

func Op(mode Mode, precision types.Precision) OpFaceRecovery {
	return OpFaceRecovery{
		id:        fmt.Sprintf("face-recovery_%s_%s", mode, precision),
		mode:      mode,
		precision: precision,
	}
}

// Mode is the type of upscale operation.
type Mode string

// Constants for the face recovery modes.
const (
	ModeRealistic Mode = "realistic"
	ModeCreative  Mode = "creative"
)
