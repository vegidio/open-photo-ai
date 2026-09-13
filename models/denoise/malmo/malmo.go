package malmo

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/denoise"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to malmo; the shared implementation lives in the denoise package.
var variant = &denoise.Variant{
	Codename: "malmo",
	Label:    "Malmo",
	Profile:  profile,
}

// profile keeps the fp16 export off the Neural Engine, for the reason gothenburg, moscow and novgorod do: all four are
// the same Restormer backbone, and the op mix that decides this is a property of the architecture rather than of any
// one checkpoint. The 44 blocks are mostly layer normalization, reshape and transpose around a channel-attention
// matmul, which is not what the Neural Engine is built for, and CoreML takes it anyway whenever ALL permits.
//
// Measured on an M2 Max, macOS 26.6, ONNX Runtime 1.26, per 256x256 tile, against the re-export described below:
//
//	MLComputeUnits          fp32                fp16
//	ALL (default)           147.0ms             197.9ms
//	CPUAndGPU               146.8ms   (-0.1%)   138.6ms  (-29.9%)
//	CPUAndNeuralEngine     1072.6ms  (+630%)    302.0ms  (+52.6%)
//
// FastPrediction and sequential both measured inside the run-to-run spread and are absent rather than untried.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// The numbers above are measured against a RE-EXPORT, and the profile is the smaller half of what that export was
// worth. The graph that shipped before it was taken by CoreML as 92 partitions rather than one, costing about -62% per
// tile in both precisions and leaving fp16 49% SLOWER than fp32. Two rewrites fix it, and the next Restormer should be
// checked for both before it is measured for anything else:
//
//   - 88 ReduceL2. Restormer's attention calls F.normalize on q and k, which exports to ReduceL2, and the CoreML EP
//     has no builder for it - two per block, 44 blocks. Spelled out as Pow/ReduceSum/Sqrt/Clip/Div it is the same
//     value in ops CoreML does support.
//   - 6 Reshape and 3 Transpose. The three Downsample layers are Conv3x3 followed by PixelUnshuffle(2), which
//     decomposes to rank-6 Reshape/Transpose; CoreML refuses any tensor above rank 5. What fixes it is folding the
//     shuffle into the convolution: PixelUnshuffle puts y[c, 2h+i, 2w+j] at output channel 4c+2i+j, and y is a padded
//     3x3 convolution, so scattering that 3x3 kernel to offset (i,j) inside a zeroed 4x4 kernel and running it at
//     stride 2 reproduces the pair exactly in one node, one output channel per (c,i,j).
//
// Both are exact in real arithmetic. A third rewrite is in this export and is NOT part of the recipe - normalizing
// LayerNorm over the channel axis in NCHW rather than through to_3d/to_4d - which is bit-identical and 176 nodes
// smaller but measures as a tie; it is kept only because the numbers above were measured against it.

// New loads the malmo session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*denoise.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a malmo operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) denoise.Op {
	return variant.Op(intensity, precision)
}
