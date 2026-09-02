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
//
// # Execution mode
//
// The sequential mode is the other half of this profile, and it is not a CoreML setting: ONNX Runtime ran every
// session in this codebase with ExecutionModeParallel, and santorini's graph has nothing for that to parallelise.
// It is a StyleGAN decoder hanging off a U-Net - a backbone, not a branchy graph - so the inter-op pool never has two
// independent nodes to hand out and only charges the handoff. Isolating Session.Run on an RTX 5090 (driver 610.88,
// ONNX Runtime 1.26, CUDA provider, 512x512 input, median of 40 runs over 3 interleaved rounds):
//
//	                    fp16               fp32
//	Parallel            11.02ms            17.18ms
//	Sequential          10.14ms (1.09x)    16.51ms (1.04x)
//
// The output is bit-identical in both precisions, which is what makes this safe to set rather than a trade. The same
// comparison on the CPU provider is 288.6ms against 265.1ms, so this is the graph's shape rather than anything about
// CUDA - the provider only decides how much the handoff costs.
//
// CoreML agrees at fp16 and is a tie at fp32. Run with `go test -tags coremlbench -run
// TestCoreMLExecutionModeSantorini ./internal/utils/` on the M2 Max above, isolating Run over 12 blocks of 10 runs
// across four interleaved rounds:
//
//	                    fp32                fp16
//	Parallel            109.0ms             95.0ms
//	Sequential          109.5ms (+0.5%)     90.5ms (-4.8%)
//
// End to end through perftest the fp16 margin is much wider than the graph alone accounts for - 228.9ms against
// 206.6ms over the 640x640 sample's two faces, -9.8%, holding across four interleaved rounds with the two ranges not
// overlapping. Twice the graph's 2.4ms is 4.8ms, not 22ms, so most of that is not the graph: the inter-op pool is
// process-wide, and in parallel mode it competes with the goroutines doing this model's align, blend and paste work
// on the same cores. That part only shows up in the app's own pipeline, which is the reason to keep an end-to-end
// number here next to the isolated one rather than trusting either alone.
//
// # What the CUDA provider options cannot do for it
//
// Nothing in cudaOptions moves this graph, and two entries would break it, so the profile deliberately leaves them
// alone. Measured the same way at fp32, against the 17.18ms parallel baseline: cudnn_conv_use_max_workspace=0,
// arena_extend_strategy=kSameAsRequested, do_copy_in_default_stream=0, use_ep_level_unified_stream=1 and
// tunable_op_enable=1 all land within noise; cudnn_conv_algo_search=DEFAULT costs 2.1x (36.3ms) because all 95 Convs
// then log "running in Fallback mode" rather than taking a searched algorithm; and use_tf32=0 costs 1.5x (25.3ms).
// EXHAUSTIVE and TF32, which is what cudaOptions already asks for, are the right answers for this graph.
//
// The two that break it are worth naming, because both look like free wins from the option list. prefer_nhwc=1 fails
// at Run - "Input channels C is not equal to kernel channels * group. C: 512 kernel channels: 3" - which is exactly
// the modulated-convolution hazard EPProfile.CudaPreferNHWC describes: 23 of santorini's 95 Convs take a weight
// computed by a Reshape rather than an initializer. fuse_conv_bias=1 costs 1.4ms at fp32 and at fp16 fails outright
// in the cuDNN frontend with HEURISTIC_QUERY_FAILED.
func profileFor(types.Precision) utils.EPProfile {
	return utils.EPProfile{
		CoreMLSpecialization: utils.CoreMLSpecializationFastPrediction,
		ExecutionMode:        utils.ExecutionModeSequential,
	}
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
