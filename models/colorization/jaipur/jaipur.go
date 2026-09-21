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
	Spec:     colorization.RgbGraph,
	Profile:  profile,
}

// profile keeps jaipur's fp16 export off the Neural Engine, which on this graph is worth roughly 4.5x:
//
//	fp16 ALL   955ms        fp16 CPUAndGPU   209ms        fp16 CPUAndNeuralEngine   235ms
//
// CPUAndNeuralEngine being four times FASTER than ALL is what identifies the cost: it is not the Neural Engine
// executing the graph, it is CoreML's planner splitting the graph across the GPU and the Neural Engine and paying a
// transition at every crossing. Pinning either one alone avoids it; ALL is the only setting that cannot.
//
// Worth recording because jaipur contradicts the op-mix reasoning in CoreMLComputeUnits: a DeOldify U-Net over a
// ResNet34 body is as convolutional as they come - 57 Convs and 55 Relus out of 197 nodes - and it still wants ALL
// off. So op mix alone is not a reason to leave a graph on the default. Same answer as delhi and mumbai, but measured
// on this graph rather than inherited from them - those are DDColor, a different architecture.
//
// Sequential and FastPrediction were both swept here and sit inside the noise floor, bit-identical.
//
// Export note: Jaipur is DeOldify's ARTISTIC generator - a ResNet34 body with DynamicUnetDeep at nf_factor 1.5 - not a
// newer training run of the ResNet101/DynamicUnetWide graph that shipped before it, so anything measured against that
// one is not comparable. fastai's PixelShuffle_ICNR blurs with a ReplicationPad2d and MLProgram supports only
// `constant` and `reflect` padding, so those five Pads have to be rewritten as a Slice+Concat of the border row and
// column BEFORE tracing to get one CoreML partition. Same trap as mumbai.
// The two 3x3 convolutions at the top of the decoder - 303 channels in and out, at the full 560x560 - are the one
// place in the published graphs where WebGPU's Vulkan path goes wrong: a second such convolution consuming the first
// one's output hangs the GPU outright on AMD's Mesa driver (the kernel resets the ring, the session returns garbage or
// blocks on the lost device). 303 is not a multiple of four, so the provider takes its unvectorised convolution
// there; the same two layers at 304 channels are fine, as is either one alone. Until the plugin is fixed, or the
// graph is re-exported with padded channels, they run on the CPU. That costs about half the graph's work - these two
// layers are 55% of its multiply-adds - so jaipur gains far less from the GPU than the other families do, but it
// gains rather than hangs.
var webgpuOptions = map[string]string{
	"forceCpuNodeNames": "/m/layers.10/layers.0/layers.0.0/Conv\n/m/layers.10/layers.1/layers.1.0/Conv",
}

func profile(precision types.Precision) utils.EPProfile {
	p := utils.EPProfile{WebGPUOptions: webgpuOptions}

	if precision == types.PrecisionFp16 {
		p.CoreMLComputeUnits = utils.CoreMLComputeUnitsCPUAndGPU
	}

	return p
}

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
