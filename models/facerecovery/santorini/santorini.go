package santorini

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to santorini; the shared implementation lives in the facerecovery package.
var variant = &facerecovery.Variant{
	Codename: "santorini",
	Label:    "Santorini",
	// The graph takes the image alone, so there is no fidelity weight to bind; -1 marks its absence.
	Inputs:   []string{"input"},
	TileSize: 512,
	Fidelity: -1,
	Profile:  profileFor,
}

// profileFor asks CoreML to specialize in Santorini's graph for latency.
//
// Measured through this code path on an M2 Max (macOS 26.6, ONNX Runtime 1.26), the 640x640 sample with two faces,
// median of 15 runs:
//
//	                    fp16 (SD)          fp32 (HD)
//	Default             237.8ms            259.3ms
//	FastPrediction      229.0ms (1.04x)    259.2ms
//
// The fp32 row is a tie on steady state - the minimums match to the digit across three interleaved rounds - but
// FastPrediction did cost it 20-30ms of cold start in every one of them, which is the specialization time Apple
// names as this hint's price. That is paid once per session build and the registry keeps models resident, so it is
// set for the graph rather than per tier. Both figures are one machine's, and the fp16 margin is small enough that
// another chip could erase it.
//
// Note what is deliberately *not* set here. Athens keeps itself off the Neural Engine, and Santorini is the same
// kind of model, but carrying that over costs about 2%: ALL is 229.0ms at fp16 against CPUAndGPU's 233.7ms. The
// difference is the export, not the architecture - Santorini's fp16 graph runs fp16 through the upsampling path, so
// the Neural Engine takes it whole. Before these weights were re-exported its Resize nodes were held at fp32, 84
// Cast nodes wrapping all 42 upsamples, and ALL measured 324.6ms against CPUAndGPU's 244.0ms - the Athens rule
// looked right for it too. If the weights are re-exported again, that ordering is the thing to re-measure.
func profileFor() utils.EPProfile {
	return utils.EPProfile{CoreMLSpecialization: utils.CoreMLSpecializationFastPrediction}
}

// New loads the santorini session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*facerecovery.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a santorini operation at the given precision, for the given pre-detected faces.
func Op(precision types.Precision, faces []detection.Face) facerecovery.Op {
	return variant.Op(precision, faces)
}
