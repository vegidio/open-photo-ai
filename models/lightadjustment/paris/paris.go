package paris

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/lightadjustment"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to paris; the shared implementation lives in the lightadjustment package.
//
// # Which weights these are
//
// Paris is IAT (arXiv 2205.14871), the IAT_enhance variant, from the `best_Epoch_exposure.pth` checkpoint rather
// than the `best_Epoch_lol_v1.pth` one the same release ships. Nothing in the original export recorded which, so it
// was recovered by matching the graph's initializers against both: 67 of the 81 tensors with 50 or more elements are
// bit-identical to the exposure checkpoint and none at all to lol_v1, which share no tensor with each other. The 14
// unmatched are the conv_large BatchNorms, folded into their convolutions at export. That also agrees with the
// upstream img_demo.py, where `--task exposure` loads exactly that file.
//
// # Why the graph is a quarter of the size the architecture implies
//
// IAT's local branch is six CBlock_ln transformer blocks. In the published weights - BOTH checkpoints, so this is a
// property of the release and not of a bad download - every one of those blocks has its Aff_channel (alpha, beta,
// color), conv1, conv2, attn and mlp tensors stored as float32 denormals around 4e-41. They underflow to zero the
// moment they multiply anything, so each block's two residual branches contribute either exactly 0 (mul_blocks) or a
// per-channel constant of at most 2.3e-4 (add_blocks, whose gammas survived). What is left of a block is
// `x + pos_embed(x)`, a depthwise 3x3.
//
// So the export folds the stack down to what it computes: one 3x3 lift, three depthwise 3x3 convolutions per path
// with the constants folded into their biases, and the two end convolutions. Measured against the previous graph at
// four resolutions, that is 136-140 dB - float round-off, not an approximation - and it takes the graph from 320
// nodes and 410 KB to 78 and 280 KB.
//
// This folding is specific to these weights. Retrain IAT, or fine-tune from a checkpoint whose blocks are alive, and
// it is wrong: the correct move then is to re-export the unfolded architecture, not to patch this one.
//
// # Why the canvas is a fixed square
//
// The graph paris shipped before had dynamic spatial axes and a dynamic batch, and on macOS that meant it did not
// run on CoreML at all. With RequireStaticInputShapes on - which is what a variant declaring no profile gets - the
// provider declined all but 4 of its 272 nodes and the rest ran on CPU kernels; with it off the session failed to
// build outright ("axis 4 is not in valid range [-4,3]"). Measured on an M2 Max (ORT 1.26, macOS 26.6) at 1024x688,
// the shipping graph landed on the same number through CoreML as through the CPU provider, in both precisions,
// which is the tell:
//
//	              CoreML      CPU provider
//	fp32          650ms       646ms
//	fp16          634ms       632ms
//
// No provider setting reaches that, so the fixed shape is not a tuning choice but the only way onto the GPU. At a
// fixed 1024x1024 the folded graph is a single CoreML partition (fp32; fp16 adds only the trailing fp32 output
// Cast, as lyon's does) and the same sweep gives 10.3ms fp32 and 11.9ms fp16 - a 98% cut, and understated, since the
// square is 1.49x the pixels of the row it is compared against. The CPU provider gains too, 646ms to 88ms, which is
// what makes this the right shape on Windows and Linux as well rather than a Mac-only trade.
//
// # Why 1024, and what it costs
//
// A fixed square is not output-neutral, and the reason is not the padding. IAT's global branch reduces over the
// whole canvas to produce one gamma and one colour matrix, so its answer moves with the scale it is shown: the same
// 640x640 image fed at 1024 shifts the result by +7.6 levels with no padding involved at all. Reflect, wrap, edge
// and whole-image pad fills all measured within noise of each other, so ReflectionPad stays as it is.
//
// That makes the canvas size the whole decision, and it is settled by staying near the scale paris already uses.
// Over 27 crops above the old 1024 ceiling, against what paris renders today:
//
//	canvas    PSNR median    worst      DC median    worst
//	1024      41.9 dB        31.7 dB    0.82         5.61 levels
//	768       31.2 dB        24.7 dB    3.34        11.83 levels
//	512       27.3 dB        24.2 dB    3.49        12.01 levels
//
// 1024 wins because it is the size those images were already being resized to; only the padding is new. Images
// BELOW the old ceiling change more, and that is the real cost of this: they used to run at their native size and
// now get resampled to the canvas like everything else.
var variant = &lightadjustment.Variant{
	Codename: "paris",
	Label:    "Paris",
	Canvas:   lightadjustment.Canvas{Size: 1024},
	Profile:  profile,
}

// profile puts paris's fp16 graph on the GPU and leaves fp32 on the provider defaults.
//
// Measured on an M2 Max at the 1024x1024 canvas, ONNX Runtime 1.26, CoreML MLProgram, medians of 18 blocks of five
// runs with the run order rotated between rounds so position bias inside the sweep cancels:
//
//	           ALL (default)    CPUAndGPU              CPUAndNeuralEngine
//	fp32       9.460ms          9.452ms (a tie)        91.9ms (+872%)
//	fp16       18.275ms         9.241ms (-49.4%)       31.4ms (+72%)
//
// The fp16 row is why this function exists: ALL costs twice what the GPU alone does, and it is also what turns fp16
// from slower than fp32 into a tie with it. fp32 is left alone deliberately - CPUAndGPU reproduces the default to
// within 0.1%, and a setting that only ever reproduces the default is one that will be carried somewhere it does not
// belong.
//
// The Neural Engine loses in both precisions, and badly. That is the same answer lyon gives, on the same hardware,
// for a graph of a completely different shape - but the two were measured separately, and neither is evidence for
// the other.
//
// The fp16 export also takes the sequential execution mode, which is a much smaller number and is here on the
// strength of its consistency rather than its size: -2.3% and -2.9% on CoreML in the two build orders, -0.4% on the
// CPU provider, and the lowest minimum of any row in all three. That is the same direction every other fp16 graph in
// this codebase reports - see ExecutionMode in ep_profile.go - and it is 0.2ms, so it is worth having and worth
// nobody's time to defend. fp32 does not get it: there it measures +1.2% on CoreML against -2.7% on the CPU
// provider, which is a wash rather than a setting.
//
// Measured against, and not set: SpecializationStrategy=FastPrediction and AllowLowPrecisionAccumulationOnGPU, both
// within 0.8% in both precisions and both build orders.
var profile = utils.Fp16Only(utils.EPProfile{
	CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU,
	ExecutionMode:      utils.ExecutionModeSequential,
})

// New loads the paris session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*lightadjustment.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a paris operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) lightadjustment.Op {
	return variant.Op(intensity, precision)
}
