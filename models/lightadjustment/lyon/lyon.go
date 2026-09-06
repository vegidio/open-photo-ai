package lyon

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/lightadjustment"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to lyon; the shared implementation lives in the lightadjustment package.
//
// # Why the canvas is a fixed square
//
// Lyon is CIT-EC (arXiv 2309.04366), a 27.4M-parameter window-attention transformer: 4 residual groups of 6 blocks,
// window size 8, on an H/4 x W/4 feature grid. Two properties of that architecture decide how it has to be run, and
// both were measured before anything was built.
//
// It cannot be tiled. Every one of the 24 blocks contains a channel-attention block whose AdaptiveAvgPool2d and a
// half-instance-norm block whose InstanceNorm2d both reduce over the whole spatial extent - the paper's own words are
// that they are there "to acquire the global statistics". Give each tile its own statistics and each tile gets its own
// exposure correction. Measured through the real graph on two photos, with this repo's exact TileGrid geometry and
// blending:
//
//	                    per-tile DC spread    PSNR vs whole-image    seam ratio at tile boundaries
//	256px tiles / 16    115.8 levels          16.1 dB                1.60x
//	512px tiles / 32     50.9 levels          18.9 dB                1.31x
//
// No overlap width fixes a DC offset, so the whole image goes through in one pass and the low-resolution result is
// applied as a gain map at full resolution, the way paris does it.
//
// It cannot be exported with dynamic axes either, though not for the reason one would expect. A dynamic export is
// numerically perfect - the tracer turns calculate_mask into real ops rather than baking a constant, and the result is
// pixel-identical to PyTorch at every resolution tried (123-125 dB, max 0.001 levels). It is CoreML that refuses it:
// with h and w unbounded, every reshape and slice in a window-attention graph has an unbounded dimension, MLProgram
// reports "has unbounded dimension which is not supported" 720 times, the graph splits into 363 partitions and then
// fails at run time. Hence one fixed 1024x1024 canvas.
//
// # Why 1024
//
//	canvas    file       CoreML    CPU        vs whole-image reference
//	512       116.9 MB   192 ms    2289 ms    23.1 dB on the over-exposed sample, -16.1 levels DC
//	1024      129.5 MB   830 ms    9280 ms    46.5 dB, -1.0 levels DC
//
// 512 is four times faster and renders a visibly different photo - a 16-level global darkening on the over-exposed
// sample, which is not a subtlety. 1024 costs 12.6 MB more, not the 150 MB a bigger canvas would suggest, because the
// twelve baked shifted-window masks are bit-identical and share one initializer.
//
// # Export notes for anyone rebuilding these weights
//
// Two graph rewrites are load-bearing, and without them CoreML is slower than the CPU:
//
//	                                                                    CoreML partitions
//	as exported                                                         73
//	window_partition/window_reverse rewritten to stay within rank 5      25
//	q/k/v taken by Split (chunk) instead of three Gather                  1
//
// CoreML rejects any tensor of rank > 5, and the stock window_partition builds a rank-6 view. Both rewrites are
// numerically exact - verified bit-identical, 0.000000 levels - and together they take CoreML from 2506 ms to 830 ms.
// Load params_ema, not params; the checkpoint carries both.
var variant = &lightadjustment.Variant{
	Codename: "lyon",
	Label:    "Lyon",
	Canvas:   lightadjustment.Canvas{MaxSize: 1024, Square: true},
	Profile:  profileFor,
}

// profileFor puts lyon's fp16 graph on the GPU and leaves fp32 on the provider defaults.
//
// Measured on an M2 Max at the 1024x1024 canvas, ONNX Runtime 1.29, CoreML MLProgram, medians of
// nine runs with the run order rotated between rounds so position bias inside the sweep cancels.
// PSNR is against the fp32 graph on the CPU provider:
//
//	           ALL (default)          CPUAndGPU              CPUAndNeuralEngine
//	fp32       874ms / 129.6 dB       879ms (+1%, a tie)     5659ms (+547%) / 124.8 dB
//	fp16       1827ms / 64.0 dB       736ms (-60%) / 70.5    16170ms (+785%) / 49.9 dB
//
// The fp16 row is the reason this function exists, and it is unusual in being a win on both axes:
// CPUAndGPU is 60% faster *and* 6.5 dB more accurate than the default, and it turns fp16 from
// slower than fp32 (1827ms against 874ms) into faster than it (736ms). Whatever ALL picks for this
// graph at fp16, it is neither the quick nor the accurate option.
//
// fp32 is left alone deliberately. CPUAndGPU measures within 1% of the default, which is noise on
// this machine, and a setting that only ever reproduces the default is a setting that will be
// carried somewhere it does not belong.
//
// Both precisions collapse on CPUAndNeuralEngine, and the log says why rather than leaving it to
// inference: "MILCompilerForANE error: failed to compile ANE model using ANEF" - the ANE will not
// take this graph at all, so the figures above are the fallback path, not the ANE.
//
// Anyone re-measuring this must first confirm the graph is still one CoreML partition. Before the
// window-partition and qkv rewrites described above it was 207, and at that point CoreML was
// slower than the CPU provider (2506ms against 2311ms) - which would make every row of this table
// a measurement of partition handoff rather than of compute units.
func profileFor(precision types.Precision) utils.EPProfile {
	if precision == types.PrecisionFp16 {
		return utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU}
	}

	return utils.EPProfile{}
}

// New loads the lyon session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*lightadjustment.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a lyon operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) lightadjustment.Op {
	return variant.Op(intensity, precision)
}
