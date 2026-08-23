package utils

import (
	"github.com/cockroachdb/errors"
	ort "github.com/yalue/onnxruntime_go"
)

// RunUnary runs a session that takes one input tensor and produces one output tensor, returning a copy of the output.
//
// The tensor lifecycle - create input, create empty output, run, copy the data out, destroy both - was written out by
// hand in six places, each with its own wording for the same four errors. Every one of them is this function.
//
// The result is copied rather than handed back as a view: the output tensor's buffer belongs to ONNX Runtime and is
// freed by the deferred Destroy, so returning a slice into it would hand the caller memory that is already gone.
func RunUnary(session *Session, in []float32, inShape, outShape ort.Shape) ([]float32, error) {
	inTensor, err := ort.NewTensor(inShape, in)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create the input tensor")
	}
	defer inTensor.Destroy()

	return RunSession(session, []ort.Value{inTensor}, outShape)
}

// RunSession runs a session over already-built input tensors and returns a copy of its single output.
//
// Separate from RunUnary for the graphs that take more than one input - a diffusion step's timestep, a restorer's
// fidelity weight - which build their extra tensors themselves but share everything from the output tensor onwards.
// The caller owns the input tensors it passed in and must destroy them; this owns only the output.
func RunSession(session *Session, inputs []ort.Value, outShape ort.Shape) ([]float32, error) {
	outTensor, err := ort.NewEmptyTensor[float32](outShape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create the output tensor")
	}
	defer outTensor.Destroy()

	if err = session.Run(inputs, []ort.Value{outTensor}); err != nil {
		return nil, errors.Wrap(err, "failed to run the session")
	}

	// The tensor's buffer is freed by the deferred Destroy, so the data has to be copied out before returning.
	data := outTensor.GetData()
	out := make([]float32, len(data))
	copy(out, data)

	return out, nil
}
