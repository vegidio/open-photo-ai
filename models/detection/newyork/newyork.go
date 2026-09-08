package newyork

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to newyork; the shared implementation lives in the detection package.
var variant = &detection.Variant{
	Codename: "newyork",
	Label:    "New York",
	Outputs:  []string{"loc", "conf", "landmarks"},
	Profile:  profileFor,
}

// profileFor runs New York in NHWC on CUDA at both precisions, and runs the graph sequentially.
//
// # CUDA
//
// Both settings are on for both precisions, which is the part worth reading before copying anything here to another
// model: athens sets CudaPreferNHWC for fp16 only because in fp32 it measured +8%, and the same flag on its own is
// -13.9% on this graph in fp32. The layout is not a property of the precision, it is a property of what cuDNN has
// kernels for on the shapes the graph actually asks for - so it has to be measured per graph, in both precisions,
// and the answer here is the opposite of the answer next door.
//
// The two settings are independent and compose: NHWC removes a transpose pair around every convolution, and
// sequential drops the inter-op handoff that a 176-node backbone has no branch wide enough to pay for. Together they
// are worth -17.5% at fp32 and -35.1% at fp16 on the graph alone, -11.6% and -23.4% end to end on an RTX 5090.
//
// ExecutionMode is not per-provider, so sequential also reaches TensorRT, which is what the auto chain picks first
// on Windows and Linux. That was measured rather than assumed and lands inside the run-to-run spread in both
// precisions. TensorRT schedules its own engine, so there is nothing there for the inter-op pool to have been doing -
// it is a tie, not a win, and the reason to record it is that it is not a loss.
//
// Output is unaffected. Through the decode the app uses, both precisions return the same two faces with every box
// and landmark inside a tenth of a pixel of the NCHW result, holding across 20 consecutive runs on one session -
// which is the check that matters, since the failure mode this graph has actually shown was a session that went
// wrong only from its second Run on.
//
// The rest of the CUDA knobs measure as ties on top of that pair and are left at the defaults in cudaOptions:
// cudnn_conv_use_max_workspace, do_copy_in_default_stream, arena_extend_strategy, use_ep_level_unified_stream,
// tunable_op_enable, session.intra_op.allow_spinning and an intra-op pool of one all land within 1%.
// cudnn_conv_algo_search=HEURISTIC is also a tie, so EXHAUSTIVE stays for the fp32 accuracy it does not cost;
// DEFAULT is +200%, and it says why in the log - it drops 65 of the Convs into cuDNN's "running in Fallback mode"
// path. Two must stay off: use_tf32=0 costs 114% in fp32 and 29% in fp16, and fuse_conv_bias=1 is broken on this
// graph - in fp32 it is 16% slower and decodes zero faces from intact conf scores, and in fp16 it does not run at
// all, failing the first Conv with CUDNN_FE HEURISTIC_QUERY_FAILED.
//
// # CoreML
//
// Nothing is set: measured, every CoreML setting this codebase can express is a tie or a loss for this graph, so the
// provider defaults are the tuning. Two results are worth keeping. CPUAndGPU is the setting most likely to be copied
// over from athens, which does keep itself off the Neural Engine - here it costs the fp16 graph 20%, because
// RetinaFace is a ResNet34 backbone plus an FPN and three 1x1 convolutional heads, which is dense convolution end to
// end and exactly what that unit is built for. And CPUAndNeuralEngine at fp32 is 5x slower, the reminder that the
// Neural Engine is fp16-only: with the GPU withheld the fp32 graph has nowhere to go but the CPU.
//
// ExecutionMode is not per-provider, so the sequential setting above reaches CoreML too. That is measured rather
// than assumed: -6.5% at fp16 and a tie at fp32. This is the largest CoreML win the mode has in the catalogue, and
// it is the fp16 half alone. Both rows are bit-identical to the parallel result, so it is a setting rather than a
// trade here too.
//
// # Export
//
// What actually made this model fast was the export, not the provider. The weights this variant shipped with were
// exported with dynamic batch, height and width axes even though detection only ever runs the fixed 640x640 in
// TargetSize. RequireStaticInputShapes is on by default, so CoreML took 10 of 176 nodes and the other 166 ran on CPU
// kernels: 158ms a run. Re-exported at that fixed shape it is 12.3ms. Anyone re-exporting these weights must keep
// the input shape static - a dynamic-axes export costs 13x here and reports nothing but a slow run.
func profileFor(_ types.Precision) utils.EPProfile {
	return utils.EPProfile{
		CudaPreferNHWC: true,
		ExecutionMode:  utils.ExecutionModeSequential,
	}
}

// New loads the New York session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*detection.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a New York operation at the given precision.
func Op(precision types.Precision) detection.Op {
	return variant.Op(precision)
}
