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
// FastPrediction, sequential and AllowLowPrecisionAccumulationOnGPU all measured inside the run-to-run spread.
// AllowLowPrecisionAccumulationOnGPU has no field on EPProfile and was reached through the provider options directly
// rather than by adding one - the order to keep, since a typed field for a setting no model uses is harder to delete
// than it was to add. This graph is the strongest case for it, being fp16 and entirely on the GPU, and it still
// measures nothing.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// The numbers above are measured against a RE-EXPORT, and the profile is the smaller half of what that export was
// worth. The graph that shipped before it was taken by CoreML as 48 partitions rather than one, costing -58.6% per
// tile in both precisions and leaving fp16 57% SLOWER than fp32. Two rewrites fix it, and they are the trap to avoid
// on the next Restormer:
//
//   - 88 ReduceL2. Restormer's attention calls F.normalize on q and k, which exports to ReduceL2, and the CoreML EP
//     has no builder for it - two per block, 44 blocks. Spelled out as Pow/ReduceSum/Sqrt/Clip/Div it is the same
//     value in ops CoreML does support.
//   - 6 Reshape and 3 Transpose. The three Downsample layers are Conv3x3 followed by PixelUnshuffle(2), which
//     decomposes to rank-6 Reshape/Transpose; CoreML refuses any tensor above rank 5, and this ONNX Runtime's Reshape
//     and Transpose builders in fact refuse rank 5 too, so a rank-5 rewrite does not help either. What does is folding
//     the shuffle into the convolution: PixelUnshuffle puts y[c, 2h+i, 2w+j] at output channel 4c+2i+j, and y is a
//     padded 3x3 convolution, so scattering that 3x3 kernel to offset (i,j) inside a zeroed 4x4 kernel and running it
//     at stride 2 reproduces the pair exactly in one node, one output channel per (c,i,j).
//
// Both are exact in real arithmetic, landing at 1.4e-06 against the stock architecture on the same checkpoint.

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
