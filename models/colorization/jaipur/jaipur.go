package jaipur

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/colorization"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to jaipur; the shared implementation lives in the colorization package.
var variant = &colorization.Variant{
	Codename: "jaipur",
	Label:    "Jaipur",
	Spec:     colorization.DeOldify,
	Profile:  profile,
}

// profile keeps jaipur's fp16 export off the Neural Engine, which on this graph is worth roughly 4.5x.
//
// # Why the Neural Engine is wrong for it
//
// CoreMLComputeUnitsAll documents the Neural Engine as the right default because most of these graphs are
// convolutional, and jaipur is as convolutional as they come - a DeOldify U-Net over a ResNet34 body, 57 Convs and
// 55 Relus out of 197 nodes, one self-attention block, no normalization to speak of. It is the model that default's
// own reasoning predicts should want ALL. It does not:
//
//	fp16 ALL   955ms        fp16 CPUAndGPU   209ms        fp16 CPUAndNeuralEngine   235ms
//
// So the op mix is not on its own a reason to leave a graph on ALL, and a new model still has to be measured rather
// than reasoned about.
//
// CPUAndNeuralEngine is the row that says what is actually going wrong: at 235ms it is four times FASTER than ALL, so
// the cost is not the Neural Engine executing the graph, it is CoreML's planner splitting the graph across the GPU
// and the Neural Engine and paying a transition at every crossing. Pinning either one alone avoids it; ALL is the
// only setting that cannot.
//
// This is the same answer delhi and mumbai reached, but it is NOT inherited from them - those are DDColor, a
// transformer-decoder architecture with an op mix that predicts its own result. This was re-measured on jaipur's own
// graph, and again when the graph was replaced (see below), because the setting follows the graph.
//
// # Why fp32 gets nothing
//
// An MLProgram at fp32 cannot reach the Neural Engine at all, so there is no transition to avoid and the setting
// would only restate the default. Fp16Only is what keeps that from being written as one.
//
// # What else was measured and did not earn a setting
//
// ExecutionModeSequential and SpecializationStrategy=FastPrediction were both swept on this family and both came back
// inside the noise floor, FastPrediction changing sign between two sweeps (-2.9%, then +3.7%). All were bit-identical,
// so none is a correctness question either.
//
// # The graph this was measured on
//
// jaipur is exported from DeOldify's ARTISTIC generator - a ResNet34 body with DynamicUnetDeep at nf_factor 1.5 -
// which is a different architecture from the ResNet101/DynamicUnetWide graph that shipped before it, not a newer
// training run of the same one. Anything measured against the old graph is not comparable: it is 255 MB against
// 873 MB, and slower in spite of that (231ms against 179ms at fp32 on an M2 Max, and 4.16s against 3.16s on the CPU
// provider), because the parameter count sits in the backbone while the cost sits in the decoder.
//
// The export deliberately avoids the partitioning trap mumbai documents. fastai's PixelShuffle_ICNR blurs with a
// ReplicationPad2d((1,0,1,0)), and MLProgram supports only `constant` and `reflect` padding, so a naive export hands
// CoreML five Pads it cannot take and partitions into six subgraphs around them. Rewriting those five as a
// Slice+Concat of the border row and column BEFORE tracing - rather than patching the graph afterwards - gives one
// CoreML partition, 197 of 197 nodes, straight out of the exporter.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// New loads the jaipur session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorization.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a jaipur operation at the given precision.
func Op(precision types.Precision) colorization.Op {
	return variant.Op(precision)
}
