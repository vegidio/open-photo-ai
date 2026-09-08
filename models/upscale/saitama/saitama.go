package saitama

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to saitama; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Saitama",
	Codename:     "saitama",
	ScaleBuckets: upscale.DefaultScaleBuckets,
	Profile:      profile,
}

// profile puts Saitama's fp16 graph on the Neural Engine, and leaves fp32 alone.
//
// This is kyoto's profile arrived at independently, which is the expected answer for the expected reason: both are
// RRDBNets, and this one is 96 convolutions, 75 LeakyRelus and 72 concatenations with no attention anywhere - the
// dense-convolution mix the Neural Engine is built for. What makes it worth writing down separately is that the
// setting was worth nothing until the fp16 export was fixed, and against the export that shipped before it the
// Neural Engine was the WORST of the three choices rather than the best.
//
// # Why only fp16
//
// CoreML's typed execution bars an fp32 MLProgram from the Neural Engine, so at fp32 there is nothing to choose:
// MLComputeUnits=ALL already means CPU and GPU, and asking for the Neural Engine anyway drops the graph onto the CPU
// (+1849% on an M2 Max). Nothing else moves the fp32 pass either - the re-export below and CPUAndGPU are both ties.
//
// # The fp16 pass, and why the export comes first
//
// The published fp16 export left its two upsample Resize nodes in fp32, because onnxconverter-common blocks Resize
// when it converts a model to float16 - the same defect kyoto had, from the same tool. That put four Cast nodes
// around the 64-channel feature maps at 512x512 and 1024x1024, which are the largest tensors in the graph.
//
// The damage is not visible where you would look for it. ORT still hands the whole graph to CoreML as ONE partition,
// 282 of 282 nodes, on both exports - the split is inside the MLProgram, where CoreML has to move two fp32 islands
// off whichever engine is running the fp16 around them. Counting partitions says the model is fine; only a timing
// says it is not.
//
// The two exports invert the answer. On the published export the Neural Engine is the worst choice on offer, 50%
// behind doing nothing; on the re-export it is the best by a wide margin (-54%), and the GPU is unchanged at 76ms on
// both - the GPU never cared about the fp32 islands, and the Neural Engine could not get past them. Tuning the
// compute units against that export would have measured the conversion and shipped CPUAndGPU, which is a third of
// what is actually available here.
//
// ALL is not a substitute for naming the unit. It lands between the two, and its spread is the tell: 63.3ms fastest
// against a 72.9ms median, where every named configuration holds within 3ms of its own median. CoreML re-decides the
// placement, and the setting is what stops it.
//
// End to end on the 640x640 sample at 4x, fp16 goes 1.067s -> 677.7ms with the re-export -> 551.1ms with this
// profile. The export is the larger half at 1.57x and the profile the smaller at 1.23x, and neither is reachable
// without the other: the profile applied to the published export is the +50% row above. fp32 is a tie throughout.
//
// # Quality
//
// Better than what shipped, not merely acceptable: 2.790/255 worst-pixel and 69.1 dB against an fp32 CPU-provider
// reference, where the published export on ALL was 3.973/255 and 68.7 dB. The GPU is the more accurate of the two
// engines, as it is on kyoto, and 44% slower for it; the difference is not visible. The fp32 re-export is
// bit-identical to the published fp32 on the CPU provider, so the fp32 half of this costs nothing to take.
//
// # ModelFormat=NeuralNetwork, which is the trap
//
// It is the one remaining CoreML option, and on the fp32 graph it measures -38.9%, the largest single number
// anywhere in this comment. It is not set, because it is not a speedup: NeuralNetwork is the older format and has no
// typed execution, so CoreML is free to put an fp32 graph on the Neural Engine and run it in half precision - which
// is exactly what it does, matching the shipping fp16/ANE row to four significant figures in both time (53.6ms
// against 52.9ms) and accuracy (69.1 dB both). The 38.9% is not fp32 getting faster; it is fp32 quietly becoming
// fp16, at twice the download and with the precision the user asked for silently discarded.
//
// This is the reason coreMLOptions pins ModelFormat to MLProgram for every model rather than leaving it tunable. An
// option that trades accuracy for speed without saying so is one a profile should not be able to reach by accident,
// and the honest way to take that 53ms is to select the fp16 model, which is what the row above it already is.
//
// # What else was measured and left alone
//
// That exhausts the CoreML provider's options; SpecializationStrategy, AllowLowPrecisionAccumulationOnGPU,
// EnableOnSubgraphs and the execution mode are all within 2% on this graph with bit-identical output.
//
// The execution mode is worth a note because tokyo sets it and saitama, like kyoto, deliberately does not. Under
// CoreML the question cannot arise - the provider takes the whole graph as one partition, so the inter-op pool has
// one node to schedule whatever the mode says. On the CPU provider, where there really are a few hundred nodes, it
// is a wash: RRDB's dense blocks feed each concatenation from several branches at once, which is the wide,
// independent structure the pool exists for and the one SwinIR's chain of attention blocks does not have.
//
// # Portability
//
// CPUAndNeuralEngine makes ORT throw at session build on a Mac with no Neural Engine, which would drop this model to
// the CPU provider - and the CPU provider is 2.4 seconds a tile here against CoreML's 53ms, so that is not a
// degradation anyone would ship. It cannot be reached: no darwin_amd64 ONNX Runtime is published (see
// internal/artifacts.go), so every Mac that runs inference at all is Apple Silicon and has one. Publishing an Intel
// runtime would make this setting conditional - see kyoto, which carries the same dependency.
//
// The margins are this machine's, and the balance between the Neural Engine and the GPU differs across Apple Silicon
// generations. The direction follows from the op mix, which is the same everywhere, but re-measure before quoting
// the numbers on other hardware.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndNeuralEngine})

// New loads the saitama sessions for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*upscale.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a saitama operation at the given scale and precision.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}
