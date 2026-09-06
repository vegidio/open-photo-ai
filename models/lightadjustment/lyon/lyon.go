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
// Measured on an M2 Max at the 1024x1024 canvas (macOS 26.6, ONNX Runtime 1.29, CoreML MLProgram) with
// internal/utils/coreml_lyon_bench_test.go, medians of blocks with the run order rotated between rounds:
//
//	           ALL (default)     CPUAndGPU
//	fp32       827ms             827ms
//	fp16       1770ms            687ms (-61%)
//
// Every sweep behind those numbers was started from a cool machine with a three-minute pause ahead of it, and that
// is load-bearing rather than a ritual. Run back to back, the same four rows drifted to a 1950ms median against a
// 1042ms minimum and manufactured a 4% "win" for a setting that is a tie in both build orders once the machine is
// cool. The tell is the spread between a row's median and its own minimum: compare rows only where that is inside a
// percent.
//
// CoreML's own compute plan - the ProfileComputePlan provider option, which logs the device each op is assigned to -
// says what is happening, and it is worth reading before touching this function, because the two precisions come out
// the way they do for opposite reasons:
//
//	                     Neural Engine    GPU
//	fp32 ALL                         0   1988
//	fp32 CPUAndGPU                   0   1988
//	fp16 ALL                       363   1626
//	fp16 CPUAndGPU                   0   1989
//
// fp32 is not a tie that happens to land inside the noise. ALL and CPUAndGPU compile to the same plan, because an
// MLProgram at fp32 cannot reach the Neural Engine at all, so setting CPUAndGPU there would only ever restate the
// default - which is how a setting ends up carried to a model where it does not belong.
//
// fp16 is the row this function exists for. ALL scatters 363 of the graph's ops onto the Neural Engine among 1626
// GPU ops, so every run pays hundreds of ANE-to-GPU transitions, and that is the whole of the 2.6x. It is also the
// whole of the accuracy gap, because the ops that land on the ANE run at its reduced internal precision. Scored by
// TestLyonOutputQuality against the fp32 CPU-provider result:
//
//	fp32 / coreml ALL              137.4 dB
//	fp32 / coreml CPUAndGPU        137.4 dB
//	fp16 / cpu                      76.1 dB
//	fp16 / coreml ALL               69.3 dB
//	fp16 / coreml CPUAndGPU         74.0 dB
//
// So CPUAndGPU is faster AND more accurate at fp16 - it recovers most of what the provider was costing against the
// same weights on the CPU - and it turns fp16 from slower than fp32 into faster than it. The two fp32 rows landing
// on the same figure to a tenth of a dB is the same fact as their identical plan, seen from the output side.
//
// CPUAndNeuralEngine is absent from the table because the ANE will not take this graph at all: the CoreML log says
// "MILCompilerForANE error: failed to compile ANE model using ANEF", so what that setting measures is the fallback
// path, at 5.7s in fp32 and 16.2s in fp16.
//
// Nothing else CoreML exposes moves lyon. SpecializationStrategy=FastPrediction,
// AllowLowPrecisionAccumulationOnGPU=1 and a sequential execution mode were each measured on top of the compute
// units above, in both build orders and both precisions, and all of those rows land within 0.3% of the row that
// ships, with bit-identical output. There is a reason rather than an accident in each: the graph is one fused CoreML
// node, so the inter-op pool has nothing to schedule, and it is already fixed-shape and resident, which is the case
// FastPrediction is there to buy.
//
// # The one node CoreML will not take
//
// The fp16 graph is 2011 of its 2012 nodes on CoreML, and that is not worth a re-export. The export is fp32 in and
// fp32 out per the repo's convention, so it opens with a Cast that consumes the graph input, and ONNX Runtime's
// CoreML Cast builder declines a Cast that has no producer node - "Cast has no preceding nodes" in the verbose log -
// leaving a 1x3x1024x1024 fp32-to-fp16 conversion on the CPU partition. Splicing any node ahead of that Cast fixes
// the placement outright, to 2013 of 2013 with every node on CoreML; Clip(input, 0, 1) does it without needing a
// graph transformer switched off, and is the model's real input contract rather than a trick. It measures 685ms
// against the shipping graph's 686ms, at a bit-identical 74.0 dB. The fp32 graph has no such node and is a clean
// 2010 of 2010.
//
// Anyone re-measuring this must first confirm the graph is still one CoreML partition. Before the window-partition
// and qkv rewrites described above it was 207, and at that point CoreML was slower than the CPU provider (2506ms
// against 2311ms) - which would make every row of these tables a measurement of partition handoff rather than of
// compute units.
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
