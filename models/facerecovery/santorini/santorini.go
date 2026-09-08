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
// FastPrediction is worth 1.04x at fp16 on an M2 Max and is a tie at fp32 on steady state - but it did cost fp32
// 20-30ms of cold start in every round, which is the specialization time Apple names as this hint's price. That is
// paid once per session build and the registry keeps models resident, so it is set for the graph rather than per
// tier. Both figures are one machine's, and the fp16 margin is small enough that another chip could erase it.
//
// Note what is deliberately *not* set here. Athens keeps itself off the Neural Engine, and Santorini is the same
// kind of model, but carrying that over costs about 2%. The difference is the export, not the architecture -
// Santorini's fp16 graph runs fp16 through the upsampling path, so the Neural Engine takes it whole. Before these
// weights were re-exported its Resize nodes were held at fp32, 84 Cast nodes wrapping all 42 upsamples, and ALL
// measured 324.6ms against CPUAndGPU's 244.0ms - the Athens rule looked right for it too. If the weights are
// re-exported again, that ordering is the thing to re-measure.
//
// # Execution mode
//
// The sequential mode is the other half of this profile, and it is not a CoreML setting: ONNX Runtime ran every
// session in this codebase with ExecutionModeParallel, and santorini's graph has nothing for that to parallelise.
// It is a StyleGAN decoder hanging off a U-Net - a backbone, not a branchy graph - so the inter-op pool never has two
// independent nodes to hand out and only charges the handoff. It is worth 1.09x at fp16 and 1.04x at fp32 on CUDA,
// with bit-identical output in both precisions, which is what makes this safe to set rather than a trade. The same
// comparison on the CPU provider gains as well, so this is the graph's shape rather than anything about CUDA - the
// provider only decides how much the handoff costs. CoreML agrees at fp16 (-4.8%) and is a tie at fp32.
//
// End to end through perftest the fp16 margin is much wider than the graph alone accounts for - -9.8% over the
// 640x640 sample's two faces, where twice the graph's 2.4ms saving is 4.8ms and not 22ms. Most of that is not the
// graph: the inter-op pool is process-wide, and in parallel mode it competes with the goroutines doing this model's
// align, blend and paste work on the same cores. That part only shows up in the app's own pipeline, which is the
// reason to keep an end-to-end number here next to the isolated one rather than trusting either alone.
//
// # What the CUDA provider options cannot do for it
//
// Nothing in cudaOptions moves this graph, and two entries would break it, so the profile deliberately leaves them
// alone. cudnn_conv_use_max_workspace=0, arena_extend_strategy=kSameAsRequested, do_copy_in_default_stream=0,
// use_ep_level_unified_stream=1 and tunable_op_enable=1 all land within noise; cudnn_conv_algo_search=DEFAULT costs
// 2.1x because all 95 Convs then log "running in Fallback mode" rather than taking a searched algorithm; and
// use_tf32=0 costs 1.5x. EXHAUSTIVE and TF32, which is what cudaOptions already asks for, are the right answers for
// this graph.
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
