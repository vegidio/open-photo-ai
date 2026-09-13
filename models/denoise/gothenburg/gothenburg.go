package gothenburg

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to gothenburg; the shared implementation lives in the denoise package.
var variant = &denoise.Variant{
	Codename: "gothenburg",
	Label:    "Gothenburg",
	Profile:  profile,
}

// profile keeps the fp16 export off the Neural Engine, for the reason moscow and novgorod do: all three are the same
// Restormer backbone, and the op mix that decides this is a property of the architecture rather than of any one
// checkpoint. The 44 blocks are mostly layer normalization, reshape and transpose around a channel-attention matmul,
// which is not what the Neural Engine is built for, and CoreML takes it anyway whenever ALL permits.
//
// Measured on an M2 Max, macOS 26.6, ONNX Runtime 1.26, per 256x256 tile, against the re-export described below:
//
//	MLComputeUnits          fp32               fp16
//	ALL (default)           142.5ms            179.4ms
//	CPUAndGPU               142.7ms  (+0.1%)   133.3ms  (-25.7%)
//	CPUAndNeuralEngine      959.5ms  (+571%)   292.7ms  (+46.7%)
//
// The fp16 row was re-run with the build order reversed, because CoreML caches what it compiles and the second
// session of a pair builds in a fraction of the first one's time: CPUAndGPU first puts ALL at +33.7%, the same effect
// with the same sign, so it is the setting and not the ordering. The fp32 row is left on the defaults by Fp16Only
// rather than by omission - an MLProgram at fp32 cannot reach the Neural Engine at all, so CPUAndGPU there only
// restates the default, and the CPUAndNeuralEngine figure is the familiar 7x fall back to the CPU rather than a
// finding.
//
// Three settings measured as no-ops and are absent rather than untried: SpecializationStrategy FastPrediction (+1.1%
// fp32, -1.5% fp16), ExecutionMode sequential (-0.4% fp32, +2.9% fp16), and AllowLowPrecisionAccumulationOnGPU
// (-0.1% in both). All land inside the run-to-run spread.
//
// Sequential is the one worth understanding rather than just recording, since it is the largest win on other graphs
// here - tokyo takes -27% from it. It does nothing on this graph for the same reason it does nothing on petersburg:
// CoreML fuses the whole model into one node, which leaves the inter-op thread pool nothing to schedule.
//
// AllowLowPrecisionAccumulationOnGPU has no field on EPProfile and is not reachable from here. It was measured
// through the provider options directly, before adding one - the order to keep, since a typed field for a setting no
// model uses is harder to delete than it was to add. This graph is the strongest case one could make for it, being
// fp16 and entirely on the GPU after the re-export, and it still measures nothing.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// The numbers above are measured against a RE-EXPORT of this model, and the profile is the smaller half of what that
// export was worth. The graph that shipped before it was taken by CoreML as 48 partitions rather than one, and this
// is what caused that, because it is the trap to avoid on the next Restormer:
//
//   - 88 ReduceL2. Restormer's attention calls F.normalize on q and k, which exports to ReduceL2, and the CoreML EP
//     has no builder for it - two per block, 44 blocks. Spelled out as Pow/ReduceSum/Sqrt/Clip/Div it is the same
//     value in ops CoreML does support.
//   - 6 Reshape and 3 Transpose. The three Downsample layers are Conv3x3 followed by PixelUnshuffle(2), which
//     decomposes to rank-6 Reshape/Transpose; CoreML refuses any tensor above rank 5, and this ONNX Runtime's
//     Reshape and Transpose builders in fact refuse rank 5 too, so a rank-5 rewrite does not help either. What does
//     is folding the shuffle into the convolution: PixelUnshuffle puts y[c, 2h+i, 2w+j] at output channel 4c+2i+j,
//     and y is a padded 3x3 convolution, so scattering that 3x3 kernel to offset (i,j) inside a zeroed 4x4 kernel and
//     running it at stride 2 reproduces the pair exactly in one node, one output channel per (c,i,j).
//
// Both rewrites are exact in real arithmetic; against the stock architecture on the same checkpoint the re-export
// lands at 1.4e-06 max absolute difference, which is float32 rounding, and it is marginally CLOSER to the PyTorch
// reference than the graph it replaces (151.2 dB against 144.4 dB PSNR on a sample tile). The export is opset 18 and
// onnx-simplified, which the previous one was not - it was opset 17 and still carried 578 unfolded Constant nodes.
//
// What the partitioning was costing, per tile, at the provider defaults: 346.5ms to 143.6ms at fp32 and 543.5ms to
// 225.1ms at fp16, both -58.6%. Together with the profile above, fp16 goes from 543.5ms to 133.3ms.
//
// That also fixes something a user would have hit directly. On the old export fp16 was 57% SLOWER than fp32, so
// picking fp16 for speed got the opposite; on the re-export fp16 is 133.3ms against fp32's 142.5ms, and the
// precision choice behaves the way it reads.

// New loads the gothenburg session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*denoise.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a gothenburg operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) denoise.Op {
	return variant.Op(intensity, precision)
}
