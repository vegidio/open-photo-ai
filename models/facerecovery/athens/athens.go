package athens

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to athens; the shared implementation lives in the facerecovery package.
var variant = &facerecovery.Variant{
	Codename: "athens",
	Label:    "Athens",
	// CodeFormer takes the fidelity weight as a second input; 1.0 is full fidelity to the original face.
	Inputs:   []string{"input", "weight"},
	TileSize: 512,
	Fidelity: 1.0,
	Profile:  profileFor,
}

// profileFor keeps athens off the Neural Engine, runs its fp16 graph in NHWC on CUDA, and runs the graph
// sequentially.
//
// # CoreML
//
// The fp32 graph is unaffected by the compute units because the Neural Engine is fp16-only, so CoreML never had work
// to put there. The fp16 graph is a different matter: ALL makes most of the graph eligible, CoreML takes it, and the
// transitions cost more than the Neural Engine saves - CPUAndGPU is worth 1.57x on an M2 Max. CodeFormer is why - 72
// GroupNorms, 19 LayerNorms held at fp32 for precision, and several hundred reshapes and transposes through the
// transformer, which is close to the worst case for a unit built for dense convolution. CPUAndNeuralEngine is the
// tell: 10-14x slower is not a slower engine, it is an engine rejecting most of the graph and thrashing on what is
// left.
//
// Both precisions are set, though only fp16 moves. It documents the intent, and it means a future export that shifts
// what the Neural Engine will accept cannot silently re-enable it.
//
// The cause is the graph's op mix, which is the same everywhere, and the Neural Engine is close to uniform across
// Apple Silicon while the GPU is not - so the direction should hold on any Mac, and on Intel there is no Neural
// Engine and the setting is a no-op. The 1.57x is this machine's number, though; a smaller GPU narrows the gap.
//
// # Execution mode
//
// Athens runs sequentially, like every other tuned model in the catalogue, and CUDA is where that is worth something:
// -20.6% at fp32 and -13.2% at fp16 on the graph alone, diluted to -8.5% and -4.6% end to end because most of what
// perftest measures here is the align and blend around each face rather than the graph. It is free - outputs are
// bit-identical, not merely close, because the mode changes only who schedules the nodes. On TensorRT it is a tie, as
// it is for newyork, since that provider schedules its own engine.
//
// On CoreML it leans 0.1-0.3% the OTHER way, and that is measured rather than assumed - athens was swept in both
// build orders, and parallel is ahead in all four rows including the ones where it runs second, so the sign is real.
// What makes it worth recording is that athens is the only fp16 graph in the catalogue that does not prefer
// sequential on CoreML, where tokyo, santorini and newyork gain 3.5-6.5%.
//
// ExecutionMode is not per-provider, so this has to be one answer, and 0.1-0.3% is not a reason to make athens the
// one model that runs a different mode from the rest. Do not read the CoreML result as an argument for switching
// back: the number to beat is on the CUDA side, and it is two orders of magnitude larger.
//
// # CUDA
//
// NHWC is set for fp16 only, and fp32 is why it is not set for both: on an RTX 5090 it is worth -7.7% at fp16 and
// costs +5.9% at fp32. The split is the tensor cores - cuDNN's fp16 kernels are written for NHWC, so in fp16 the flag
// removes a transpose pair around every convolution and reaches those kernels, while in fp32 there are no such
// kernels to reach and the layout conversion is pure cost. CodeFormer is convolution-heavy enough for that to be
// worth measuring even though its transformer stack is not.
//
// Output quality is unaffected. Through the real pipeline - detection, align, restore, blend - NHWC lands 67.1 dB
// PSNR from the NCHW result, while both sit 56.6 dB from the fp32 model: the layout moves the answer by well under
// what choosing fp16 at all already moves it.
//
// The rest of the CUDA knobs were measured on this graph and left alone: cudnn_conv_algo_search=HEURISTIC is inside
// the noise of EXHAUSTIVE (and DEFAULT is 70% slower), do_copy_in_default_stream, arena_extend_strategy,
// use_ep_level_unified_stream, tunable_op_enable and sdpa_kernel all measure as ties, use_tf32=0 costs 37%, and
// fuse_conv_bias=1 returns a different answer on every run and must stay off.
func profileFor(precision types.Precision) utils.EPProfile {
	profile := utils.EPProfile{
		CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU,
		ExecutionMode:      utils.ExecutionModeSequential,
	}

	if precision == types.PrecisionFp16 {
		profile.CudaPreferNHWC = true
	}

	return profile
}

// New loads the athens session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*facerecovery.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds an athens operation at the given precision, for the given pre-detected faces.
func Op(precision types.Precision, faces []detection.Face) facerecovery.Op {
	return variant.Op(precision, faces)
}
