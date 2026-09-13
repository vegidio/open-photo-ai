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
// The fp16 win was confirmed in both build orders, because CoreML caches what it compiles and the second session of a
// pair builds in a fraction of the first one's time: ALL first puts CPUAndGPU at -23.4%, CPUAndGPU first puts ALL at
// +27.5%. Same effect, same sign, so it is the setting and not the ordering. The fp32 row is left on the defaults by
// Fp16Only rather than by omission - an MLProgram at fp32 cannot reach the Neural Engine at all, so CPUAndGPU there
// only restates the default, measuring within 0.2% in both orders, and the CPUAndNeuralEngine figure is the familiar
// 7x fall back to the CPU rather than a finding.
//
// Two settings measured as no-ops and are absent rather than untried: SpecializationStrategy FastPrediction (-0.1%
// fp32, -4.2% fp16) and ExecutionMode sequential (-0.3% fp32, +2.6% fp16). Both land inside the run-to-run spread.
//
// Sequential is the one worth understanding rather than just recording, since it is the largest win on other graphs
// here - tokyo takes -27% from it. It does nothing on this graph for the same reason it does nothing on gothenburg and
// petersburg: CoreML fuses the whole model into one node, which leaves the inter-op thread pool nothing to schedule.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// The numbers above are measured against a RE-EXPORT of this model, and the profile is the smaller half of what that
// export was worth. The graph that shipped before it was taken by CoreML as 92 partitions rather than one - worse than
// the 48 that cost gothenburg -58.6% - and this is what caused that. It is the same trap in the same architecture, so
// the next Restormer should be checked for both before it is measured for anything else:
//
//   - 88 ReduceL2. Restormer's attention calls F.normalize on q and k, which exports to ReduceL2, and the CoreML EP
//     has no builder for it - two per block, 44 blocks. Spelled out as Pow/ReduceSum/Sqrt/Clip/Div it is the same
//     value in ops CoreML does support.
//   - 6 Reshape and 3 Transpose. The three Downsample layers are Conv3x3 followed by PixelUnshuffle(2), which
//     decomposes to rank-6 Reshape/Transpose; CoreML refuses any tensor above rank 5. What fixes it is folding the
//     shuffle into the convolution: PixelUnshuffle puts y[c, 2h+i, 2w+j] at output channel 4c+2i+j, and y is a padded
//     3x3 convolution, so scattering that 3x3 kernel to offset (i,j) inside a zeroed 4x4 kernel and running it at
//     stride 2 reproduces the pair exactly in one node, one output channel per (c,i,j). The folded kernels are 9/16
//     zeros, which is what makes the fp32 file 5 MB larger than the graph it replaces.
//
// A third rewrite is in this export and is NOT part of the recipe: Restormer's LayerNorm is spelled to_3d, normalize
// over the last axis, to_4d, and normalizing over the channel axis in NCHW instead is bit-identical and 176 nodes
// smaller. It measures +0.1% at fp32 and -0.4% at fp16 - both ties - so it is kept only because it is what the numbers
// here were measured against. The next Restormer needs the two rewrites above and can ignore this one.
//
// Both of the rewrites that matter are exact in real arithmetic - the folded convolution is bit-identical in torch, and the normalize
// rewrite lands at 1.3e-06 max absolute difference, which is float32 rounding. Against the PyTorch reference the
// re-export is marginally CLOSER than the graph it replaces: 148.3 dB against 147.2 dB PSNR on sample tiles, and
// 151.5 dB through CoreML. The export is opset 18 and onnx-simplified, which the previous one was not - it was opset
// 18 but carried 1678 unfolded Constant nodes, 176 Shape and 220 Slice, for 5444 nodes against the re-export's 2481.
//
// What the partitioning was costing, per tile, at the provider defaults: 390.0ms to 148.0ms at fp32 (-62.1%) and
// 580.1ms to 215.5ms at fp16 (-62.8%). Together with the profile above, fp16 goes from 580.1ms to 138.6ms. Session
// build time falls with it, 20.7s to 3.2s at fp16, which is what the user waits through on first use.
//
// Almost none of that is the smaller graph: on the CPU provider, where the partition count cannot matter, the two
// exports are 1.094s against 1.019s at fp32 and 1.475s against 1.496s at fp16. The win is the partitioning.
//
// That also fixes something a user would have hit directly. On the old export fp16 was 49% SLOWER than fp32, so
// picking fp16 for speed got the opposite; on the re-export with this profile fp16 is 138.6ms against fp32's 146.8ms,
// and the precision choice behaves the way it reads.

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
