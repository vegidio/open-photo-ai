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

// profileFor asks CoreML to specialize santorini's graph for latency, and runs it sequentially.
//
// FastPrediction is worth 1.04x at fp16 on an M2 Max and a tie at fp32 on steady state, but it cost fp32 20-30ms of
// cold start in every round, which is the specialization time Apple names as this hint's price. That is paid once per
// session build and the registry keeps models resident, so it is set for the graph rather than per tier.
//
// Note what is deliberately NOT set. Athens keeps itself off the Neural Engine and santorini is the same kind of
// model, but carrying that over costs about 2%. The difference is the export, not the architecture: santorini's fp16
// graph runs fp16 through the upsampling path, so the ANE takes it whole. Before these weights were re-exported their
// Resize nodes were held at fp32, 84 Cast nodes wrapping all 42 upsamples, and the athens rule looked right for it
// too. If the weights are re-exported again, that ordering is the thing to re-measure.
//
// # Execution mode
//
// Santorini is a StyleGAN decoder hanging off a U-Net - a backbone, not a branchy graph - so the inter-op pool never
// has two independent nodes to hand out and only charges the handoff. Worth 1.09x at fp16 and 1.04x at fp32 on CUDA
// with bit-identical output, and the CPU provider gains too, so this is the graph's shape rather than anything about
// CUDA. CoreML agrees at fp16 (-4.8%) and is a tie at fp32.
//
// End to end the fp16 margin is much wider than the graph alone accounts for (-9.8% over the 640x640 sample's two
// faces, where twice the graph's 2.4ms saving is 4.8ms and not 22ms). The inter-op pool is process-wide, so in
// parallel mode it competes with the goroutines doing this model's align, blend and paste work on the same cores -
// which is the reason to keep an end-to-end number here next to the isolated one rather than trusting either alone.
//
// # CUDA provider options
//
// Nothing in cudaOptions moves this graph and two entries would break it. cudnn_conv_use_max_workspace=0,
// arena_extend_strategy=kSameAsRequested, do_copy_in_default_stream=0, use_ep_level_unified_stream=1 and
// tunable_op_enable=1 all land within noise; cudnn_conv_algo_search=DEFAULT costs 2.1x because all 95 Convs then log
// "running in Fallback mode"; use_tf32=0 costs 1.5x.
//
// The two that break it look like free wins from the option list. prefer_nhwc=1 fails at Run - "Input channels C is
// not equal to kernel channels * group" - which is exactly the modulated-convolution hazard EPProfile.CudaPreferNHWC
// describes: 23 of santorini's 95 Convs take a weight computed by a Reshape rather than an initializer.
// fuse_conv_bias=1 costs 1.4ms at fp32 and at fp16 fails in the cuDNN frontend with HEURISTIC_QUERY_FAILED.
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
