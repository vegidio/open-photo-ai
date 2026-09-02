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

// profileFor keeps athens off the Neural Engine, and runs its fp16 graph in NHWC on CUDA.
//
// # CoreML
//
// Measured through this code path on an M2 Max (macOS 26.6, ONNX Runtime 1.26), one 512x512 face, median of 7 runs
// against the fp32 result:
//
//	                       fp16                 fp32
//	MLComputeUnits=ALL     146.7ms              104.7ms
//	CPUAndGPU              93.5ms (1.57x)       104.7ms (unchanged)
//	CPUAndNeuralEngine     1.313s (14x slower)  1.017s (10x slower)
//
// The fp32 graph is unaffected because the Neural Engine is fp16-only, so CoreML never had work to put there. The
// fp16 graph is a different matter: ALL makes most of the graph eligible, CoreML takes it, and the transitions cost
// more than the Neural Engine saves. CodeFormer is why - 72 GroupNorms, 19 LayerNorms held at fp32 for precision,
// and several hundred reshapes and transposes through the transformer, which is close to the worst case for a unit
// built for dense convolution. The CPUAndNeuralEngine row is the tell: an order of magnitude is not a slower engine,
// it is an engine rejecting most of the graph and thrashing on what is left.
//
// Both precisions are set, though only fp16 moves. It documents the intent, and it means a future export that shifts
// what the Neural Engine will accept cannot silently re-enable it.
//
// # Execution mode
//
// Athens declares no ExecutionMode, unlike santorini next door, so it takes the zero value and runs PARALLEL - which
// applyProfile sets explicitly, rather than leaving to ONNX Runtime, whose own default is sequential. That is a gap
// rather than a finding, but the CoreML half of it is measured: `go test -tags coremlbench -run
// TestCoreMLExecutionModeAthens ./internal/utils/`. That test sweeps this model in BOTH build orders, which the
// other three do not need - they already ship sequential on the strength of what it is worth on CUDA, so their
// CoreML sweeps only have to show it costs nothing, whereas here a CoreML number is the only evidence there is:
//
//	                    parallel first        sequential first
//	fp32 parallel       106.280ms             106.657ms
//	fp32 sequential     106.404ms             106.975ms
//	fp16 parallel        94.379ms              94.550ms
//	fp16 sequential      94.662ms              94.737ms
//
// Parallel is ahead in all four, including in the rows where it is the session that runs second, so the sign is real
// rather than an artifact of the order - but it is 0.1-0.3%, which is a tie in any sense that matters. What makes it
// worth recording is that athens is the only fp16 graph in the catalogue that does NOT prefer sequential on CoreML,
// where tokyo, santorini and newyork gain 3.5-6.5%.
//
// A tie on CoreML is still no reason to set the mode either way. The reason santorini sets it is what it is worth on
// CUDA, and this graph has never been measured there. Anyone with a CUDA machine should try it: santorini is the
// same kind of backbone and gains 4-9%, and tokyo, which shares this model's transformer, gains far more.
//
// How far this carries to other Macs: the cause is the graph's op mix, which is the same everywhere, and the Neural
// Engine is close to uniform across Apple Silicon while the GPU is not - so the direction should hold on any Mac,
// and on Intel there is no Neural Engine and the setting is a no-op. The 1.57x is this machine's number, though. A
// smaller GPU narrows the gap, so expect less there. It has not been measured on another chip or another macOS.
//
// # CUDA
//
// NHWC is set for fp16 only, and the fp32 row is why it is not set for both. Measured on an RTX 5090 (driver 610.88,
// ONNX Runtime 1.26, CUDA EP) - the graph alone is one 512x512 face, median of 20 runs; end to end is perftest's
// median over 20 runs of the two faces in its sample, which also carries the align and blend work:
//
//	                  graph alone            end to end
//	fp16 NCHW         29.3ms                 91.6ms
//	fp16 NHWC         27.1ms (-7.7%)         87.9ms (-4.0%)
//	fp32 NCHW         45.6ms                 124.1ms
//	fp32 NHWC         48.3ms (+5.9%)         133.9ms (+7.9%)
//
// The split is the tensor cores: cuDNN's fp16 kernels are written for NHWC, so in fp16 the flag removes a transpose
// pair around every convolution and reaches those kernels, while in fp32 there are no such kernels to reach and the
// layout conversion is pure cost. CodeFormer is convolution-heavy enough for that to be worth measuring even though
// its transformer stack is not.
//
// Output quality is unaffected. Through the real pipeline - detection, align, restore, blend - NHWC lands 67.1 dB
// PSNR from the NCHW result, while both sit 56.6 dB from the fp32 model: the layout moves the answer by well under
// what choosing fp16 at all already moves it, and the two restored faces are indistinguishable at 3x zoom.
//
// The rest of the CUDA knobs were measured on this graph and left alone: cudnn_conv_algo_search=HEURISTIC is inside
// the noise of EXHAUSTIVE (and DEFAULT is 70% slower), do_copy_in_default_stream, arena_extend_strategy,
// use_ep_level_unified_stream, tunable_op_enable and sdpa_kernel all measure as ties, use_tf32=0 costs 37%, and
// fuse_conv_bias=1 returns a different answer on every run and must stay off.
func profileFor(precision types.Precision) utils.EPProfile {
	profile := utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU}

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
