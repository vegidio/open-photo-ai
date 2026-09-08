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
	return RunUnaryInto(session, in, inShape, outShape, nil)
}

// RunUnaryInto is RunUnary writing into dst rather than allocating a fresh output.
//
// It exists for the drivers that run one fixed shape thousands of times over a single image: every output is the same
// length, so allocating per call produces hundreds of megabytes of identically-shaped garbage. dst is grown when it is
// too short, so the returned slice - not dst - is the one to read.
//
// The result is still a copy for the same reason RunUnary's is: the output tensor's buffer belongs to ONNX Runtime and
// is freed on return.
func RunUnaryInto(session *Session, in []float32, inShape, outShape ort.Shape, dst []float32) ([]float32, error) {
	inTensor, err := ort.NewTensor(inShape, in)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create the input tensor")
	}
	defer inTensor.Destroy()

	return RunSessionInto(session, []ort.Value{inTensor}, outShape, dst)
}

// RunSession runs a session over already-built input tensors and returns a copy of its single output.
//
// Separate from RunUnary for the graphs that take more than one input - a diffusion step's timestep, a restorer's
// fidelity weight - which build their extra tensors themselves but share everything from the output tensor onwards.
// The caller owns the input tensors it passed in and must destroy them; this owns only the output.
func RunSession(session *Session, inputs []ort.Value, outShape ort.Shape) ([]float32, error) {
	return RunSessionInto(session, inputs, outShape, nil)
}

// RunSessionInto is RunSession writing into dst rather than allocating a fresh output. See RunUnaryInto.
func RunSessionInto(session *Session, inputs []ort.Value, outShape ort.Shape, dst []float32) ([]float32, error) {
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
	out := Grow(dst, len(data))
	copy(out, data)

	return out, nil
}

// Grow returns buf resized to exactly n elements, reallocating only when its capacity is too small.
//
// The contents of the returned slice are not cleared: every caller here overwrites all n elements immediately, and
// zeroing a multi-megabyte buffer thousands of times is the cost this is avoiding.
func Grow(buf []float32, n int) []float32 {
	if cap(buf) < n {
		return make([]float32, n)
	}

	return buf[:n]
}
