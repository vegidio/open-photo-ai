package facedetection

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
)

type OpFaceDetection struct {
	id        string
	precision types.Precision
}

func (o OpFaceDetection) Id() string {
	return o.id
}

func (o OpFaceDetection) Precision() types.Precision {
	return o.precision
}

// Compile-time assertion to ensure it conforms to the Op interface.
var _ types.Operation = (*OpFaceDetection)(nil)

func Op(precision types.Precision) OpFaceDetection {
	return OpFaceDetection{
		id:        fmt.Sprintf("face-detection_%s", precision),
		precision: precision,
	}
}
