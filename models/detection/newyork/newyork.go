package newyork

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to newyork; the shared implementation lives in the detection package.
//
// Profile is deliberately left nil: measured, every CoreML setting this codebase can express is a tie or a loss for
// this graph, so the provider defaults are the tuning. Measured through this code path on an M2 Max (macOS 26.6, ONNX
// Runtime 1.26), the 640x640 sample with two faces, median of 25 runs, three interleaved rounds:
//
//	                       fp32       fp16
//	ALL / Default          12.3ms     9.9ms
//	ALL / FastPrediction   12.3ms     9.9ms
//	CPUAndGPU              12.4ms     11.9ms
//	CPUAndNeuralEngine     59.2ms     10.0ms
//
// Two rows are worth keeping. CPUAndGPU is the setting most likely to be copied over from athens, which does keep
// itself off the Neural Engine - here it costs the fp16 graph 20%, because RetinaFace is a ResNet34 backbone plus an
// FPN and three 1x1 convolutional heads, which is dense convolution end to end and exactly what that unit is built
// for. And CPUAndNeuralEngine at fp32 is the reminder that the Neural Engine is fp16-only: with the GPU withheld the
// fp32 graph has nowhere to go but the CPU.
//
// What actually made this model fast was the export, not the provider. The weights this variant shipped with were
// exported with dynamic batch, height and width axes even though detection only ever runs the fixed 640x640 in
// TargetSize. RequireStaticInputShapes is on by default, so CoreML took 10 of 176 nodes and the other 166 ran on CPU
// kernels: 158ms a run. Re-exported at that fixed shape it is 12.3ms. Anyone re-exporting these weights must keep
// the input shape static - a dynamic-axes export costs 13x here and reports nothing but a slow run.
var variant = &detection.Variant{
	Codename: "newyork",
	Label:    "New York",
	Outputs:  []string{"loc", "conf", "landmarks"},
}

// New loads the newyork session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*detection.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a newyork operation at the given precision.
func Op(precision types.Precision) detection.Op {
	return variant.Op(precision)
}
