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
// ALL makes most of the fp16 graph eligible for the Neural Engine, CoreML takes it, and the transitions cost more
// than the ANE saves - CPUAndGPU is worth 1.57x on an M2 Max. CodeFormer is why: 72 GroupNorms, 19 LayerNorms held at
// fp32 for precision, and several hundred reshapes and transposes through the transformer. CPUAndNeuralEngine is the
// tell - 10-14x slower is not a slower engine, it is an engine rejecting most of the graph and thrashing on what is
// left.
//
// Both precisions are set, though only fp16 moves, so a future export that shifts what the Neural Engine will accept
// cannot silently re-enable it.
//
// # Execution mode
//
// Sequential is worth -20.6% at fp32 and -13.2% at fp16 on the graph alone under CUDA, diluted to -8.5% and -4.6% end
// to end because most of what perftest measures here is the align and blend around each face. Output is
// bit-identical. On TensorRT it is a tie, as it is for newyork.
//
// On CoreML it leans 0.1-0.3% the OTHER way, measured in both build orders with parallel ahead in all four rows, so
// the sign is real - athens is the only fp16 graph in the catalogue that does not prefer sequential on CoreML, where
// tokyo, santorini and newyork gain 3.5-6.5%. ExecutionMode is not per-provider, so this has to be one answer, and
// 0.1-0.3% is not a reason to make athens the one model that runs a different mode from the rest.
//
// # CUDA
//
// NHWC is set for fp16 only: on an RTX 5090 it is worth -7.7% at fp16 and costs +5.9% at fp32. The split is the
// tensor cores - cuDNN's fp16 kernels are written for NHWC, so in fp16 the flag removes a transpose pair around every
// convolution and reaches them, while in fp32 there are no such kernels to reach and the layout conversion is pure
// cost. Output is unaffected: through the real pipeline NHWC lands 67.1 dB from the NCHW result, where both sit
// 56.6 dB from the fp32 model.
//
// The rest were measured and left alone: HEURISTIC is inside the noise of EXHAUSTIVE (DEFAULT is 70% slower),
// do_copy_in_default_stream, arena_extend_strategy, use_ep_level_unified_stream, tunable_op_enable and sdpa_kernel are
// ties, use_tf32=0 costs 37%, and fuse_conv_bias=1 returns a different answer on every run and must stay off.
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
