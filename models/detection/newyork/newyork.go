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

// profileFor runs newyork in NHWC on CUDA at both precisions, and runs the graph sequentially.
//
// # CUDA
//
// Both settings are on for BOTH precisions, which is the part worth reading before copying anything here to another
// model: athens sets CudaPreferNHWC for fp16 only because in fp32 it measured +8%, and the same flag on its own is
// -13.9% on this graph in fp32. The layout is not a property of the precision, it is a property of what cuDNN has
// kernels for on the shapes the graph actually asks for - so it has to be measured per graph, in both precisions, and
// the answer here is the opposite of the answer next door.
//
// The two settings are independent and compose: NHWC removes a transpose pair around every convolution, and sequential
// drops the inter-op handoff that a 176-node backbone has no branch wide enough to pay for. Together they are worth
// -17.5% at fp32 and -35.1% at fp16 on the graph alone, -11.6% and -23.4% end to end on an RTX 5090. Output is
// unaffected, checked across 20 consecutive runs on one session - the check that matters, since the failure mode this
// graph has actually shown was a session that went wrong only from its second Run on.
//
// The rest of the CUDA knobs are ties and stay at the cudaOptions defaults. Two must stay off: use_tf32=0 costs 114%
// in fp32 and 29% in fp16, and fuse_conv_bias=1 is broken here - in fp32 it decodes zero faces from intact conf
// scores, and in fp16 it fails the first Conv with CUDNN_FE HEURISTIC_QUERY_FAILED.
//
// # CoreML
//
// Nothing is set; every CoreML setting this codebase can express is a tie or a loss here. Two results are worth
// keeping. CPUAndGPU is the setting most likely to be copied over from athens, and here it costs the fp16 graph 20%,
// because RetinaFace is a ResNet34 backbone plus an FPN and three 1x1 convolutional heads - dense convolution end to
// end, exactly what the Neural Engine is built for. And sequential, which is not per-provider and so reaches CoreML
// too, is worth -6.5% at fp16 and a tie at fp32: the largest CoreML win the mode has in the catalogue.
//
// # Export
//
// What actually made this model fast was the export, not the provider. The weights this variant shipped with had
// dynamic batch, height and width axes even though detection only ever runs the fixed 640x640 in TargetSize, so CoreML
// took 10 of 176 nodes and the other 166 ran on CPU kernels: 158ms a run against 12.3ms re-exported at the fixed
// shape. Anyone re-exporting these weights must keep the input shape static - a dynamic-axes export costs 13x here and
// reports nothing but a slow run.
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
