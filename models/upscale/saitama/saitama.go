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
// MLComputeUnits=ALL already means CPU and GPU, and asking for the Neural Engine anyway drops the graph onto the CPU.
// Measured on an M2 Max (macOS 26.6.2, ONNX Runtime 1.26) over one 256x256 tile, median of 12 blocks of 3 runs:
//
//	         ALL (default)   CPUAndGPU        CPUAndNeuralEngine
//	fp32     86.0ms          87.1ms (tie)     1675.7ms (+1849%)
//
// So the fp32 pass gets no setting. Nothing else moves it either: the re-export below is a tie at fp32 (86.9ms), and
// so is CPUAndGPU on that export (86.7ms). Four rows within 1.3% of each other is one row.
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
// Both exports, same machine and method, every row against the published export on its default ALL:
//
//	                    ALL              CPUAndGPU        CPUAndNeuralEngine
//	published export    116.1ms          76.3ms (-34%)    173.5ms (+50%)
//	re-export           72.9ms (-37%)    76.5ms (-34%)    53.2ms (-54%)
//
// Read the two rows rather than the two best numbers. On the published export the Neural Engine is the worst choice
// on offer, 50% behind doing nothing; on the re-export it is the best by a wide margin, and the GPU is unchanged at
// 76ms in both - the GPU never cared about the fp32 islands, and the Neural Engine could not get past them. Tuning
// the compute units against that export would have measured the conversion and shipped CPUAndGPU, which is a third
// of what is actually available here.
//
// ALL is not a substitute for naming the unit. It lands at 72.9ms, between the two, and its spread is the tell:
// 63.3ms fastest against a 72.9ms median, where every named configuration holds within 3ms of its own median. CoreML
// re-decides the placement, and the setting is what stops it.
//
// End to end through perftest on the 640x640 sample at 4x, median of 9 runs, which carries the tiling, the
// reflection padding and the overlap blend on top of the graph - the three rows run back to back in one sitting, so
// the machine's state is common to all of them:
//
//	        published export   re-export    re-export + this profile
//	fp32    838.1ms            826.5ms      836.7ms  (tie)
//	fp16    1.067s             677.7ms      551.1ms  (1.94x)
//
// The export is the larger half at 1.57x and the profile the smaller at 1.23x, and neither is reachable without the
// other: the profile applied to the published export is the +50% row above.
//
// # Quality
//
// Better than what shipped, not merely acceptable. Against an fp32 CPU-provider reference, worst-pixel deviation and
// PSNR over one tile:
//
//	fp16 published / coreml ALL           3.973/255   68.7 dB   <- what shipped
//	fp16 re-export / coreml ALL           2.928/255   69.3 dB
//	fp16 re-export / coreml CPUAndGPU     2.511/255   72.9 dB
//	fp16 re-export / coreml CPUAndANE     2.790/255   69.1 dB   <- what ships
//
// The GPU is the more accurate of the two engines, as it is on kyoto, and 44% slower for it; the difference is not
// visible. The fp32 re-export is bit-identical to the published fp32 on the CPU provider, so the fp32 half of this
// costs nothing to take.
//
// # ModelFormat=NeuralNetwork, which is the trap
//
// It is the one remaining CoreML option, and on the fp32 graph it measures -38.9%: 53.6ms against MLProgram's
// 87.7ms, the largest single number anywhere in this comment. It is not set, because it is not a speedup.
//
//	                                   time       worst pixel   PSNR
//	fp32 / MLProgram (ships)           87.7ms     0.001/255     135.3 dB
//	fp32 / NeuralNetwork               53.6ms     2.681/255      69.1 dB
//	fp16 / MLProgram, ANE (ships)      52.9ms     2.790/255      69.1 dB
//
// The bottom two rows are the same row. NeuralNetwork is the older format and has no typed execution, so CoreML is
// free to put an fp32 graph on the Neural Engine and run it in half precision - which is exactly what it does, to
// four significant figures in both time and accuracy. The 38.9% is not fp32 getting faster; it is fp32 quietly
// becoming fp16, at twice the download and with the precision the user asked for silently discarded.
//
// This is the reason coreMLOptions pins ModelFormat to MLProgram for every model rather than leaving it tunable. An
// option that trades accuracy for speed without saying so is one a profile should not be able to reach by accident,
// and the honest way to take that 53ms is to select the fp16 model, which is what the row above it already is.
//
// # What else was measured and left alone
//
// That exhausts the CoreML provider's options; the rest are noise on this graph, measured on top of the compute
// units above:
//
//	                                          fp32      fp16
//	SpecializationStrategy=FastPrediction     -1.5%     -0.5%
//	AllowLowPrecisionAccumulationOnGPU=1      -2.1%     -0.0%
//	EnableOnSubgraphs=1                       -1.5%     +0.0%
//	ExecutionMode sequential (CoreML)         +0.5%     +8.0%
//	ExecutionMode sequential (CPU provider)   -2.5%     +2.4%
//
// Four rows within 2% of each other, output bit-identical in all of them, is four ways of writing the same row.
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
