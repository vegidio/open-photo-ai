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
// window size 8, on an H/4 x W/4 feature grid. Two properties of that architecture decide how it has to be run.
//
// It cannot be tiled. Every one of the 24 blocks contains a channel-attention block whose AdaptiveAvgPool2d and a
// half-instance-norm block whose InstanceNorm2d both reduce over the whole spatial extent - the paper's own words are
// that they are there "to acquire the global statistics". Give each tile its own statistics and each tile gets its own
// exposure correction: measured through the real graph with this repo's exact TileGrid geometry, 512px tiles still
// leave a 50.9-level DC spread between tiles and 18.9 dB against the whole-image result. No overlap width fixes a DC
// offset, so the whole image goes through in one pass and the low-resolution result is applied as a gain map at full
// resolution, the way paris does it.
//
// It cannot be exported with dynamic axes either, though not for the reason one would expect. A dynamic export is
// numerically perfect - the tracer turns calculate_mask into real ops rather than baking a constant, and the result is
// pixel-identical to PyTorch at every resolution tried. It is CoreML that refuses it: with h and w unbounded, every
// reshape and slice in a window-attention graph has an unbounded dimension, MLProgram reports "has unbounded dimension
// which is not supported" 720 times, the graph splits into 363 partitions and then fails at run time. Hence one fixed
// 1024x1024 canvas.
//
// # Why 1024
//
// 512 is four times faster and renders a visibly different photo - 23.1 dB against the whole-image reference and a
// 16-level global darkening on the over-exposed sample, which is not a subtlety, where 1024 holds 46.5 dB and -1.0
// levels. 1024 costs 12.6 MB more, not the 150 MB a bigger canvas would suggest, because the twelve baked
// shifted-window masks are bit-identical and share one initializer.
//
// # Export notes for anyone rebuilding these weights
//
// Two graph rewrites are load-bearing, and without them CoreML is slower than the CPU. CoreML rejects any tensor of
// rank > 5, and the stock window_partition builds a rank-6 view: rewriting window_partition/window_reverse to stay
// within rank 5 takes the graph from 73 CoreML partitions to 25, and taking q/k/v by Split (chunk) instead of three
// Gather takes it to 1. Both rewrites are numerically exact - verified bit-identical - and together they take CoreML
// from 2506 ms to 830 ms. Load params_ema, not params; the checkpoint carries both.
var variant = &lightadjustment.Variant{
	Codename: "lyon",
	Label:    "Lyon",
	Canvas:   lightadjustment.Canvas{Size: 1024},
	Profile:  profile,
}

// profile puts lyon's fp16 graph on the GPU and leaves fp32 on the provider defaults.
//
// CoreML's own compute plan - the ProfileComputePlan provider option, which logs the device each op is assigned to -
// is what this function is built on, and it is worth reading before touching it, because the two precisions come out
// the way they do for opposite reasons.
//
// fp32 is not a tie that happens to land inside the noise: ALL and CPUAndGPU compile to the same plan (0 ops on the
// Neural Engine, 1988 on the GPU), because an MLProgram at fp32 cannot reach the Neural Engine at all. Setting
// CPUAndGPU there would only ever restate the default - which is how a setting ends up carried to a model where it
// does not belong.
//
// fp16 is the row this function exists for. ALL scatters 363 of the graph's ops onto the Neural Engine among 1626 GPU
// ops, so every run pays hundreds of ANE-to-GPU transitions, and that is the whole of the 2.6x (1770ms against 687ms
// on an M2 Max). It is also the whole of the accuracy gap, because the ops that land on the ANE run at its reduced
// internal precision: against an fp32 CPU reference, fp16 on ALL scores 69.3 dB and on CPUAndGPU 74.0 dB, close to
// the 76.1 dB the same weights reach on the CPU provider. So CPUAndGPU is faster AND more accurate at fp16, and it
// turns fp16 from slower than fp32 into faster than it.
//
// CPUAndNeuralEngine is absent from those figures because the ANE will not take this graph at all: the CoreML log
// says "MILCompilerForANE error: failed to compile ANE model using ANEF", so what that setting measures is the
// fallback path, at 5.7s in fp32 and 16.2s in fp16.
//
// Nothing else CoreML exposes moves lyon. SpecializationStrategy=FastPrediction,
// AllowLowPrecisionAccumulationOnGPU=1 and a sequential execution mode all land within 0.3% with bit-identical
// output, and there is a reason rather than an accident in each: the graph is one fused CoreML node, so the inter-op
// pool has nothing to schedule, and it is already fixed-shape and resident, which is the case FastPrediction is there
// to buy.
//
// Whoever re-measures this: start from a cool machine with a three-minute pause ahead of the sweep. Run back to back,
// the same four rows drifted to a 1950ms median against a 1042ms minimum and manufactured a 4% "win" for a setting
// that is a tie in both build orders once the machine is cool. The tell is the spread between a row's median and its
// own minimum; compare rows only where that is inside a percent.
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
// against 2311ms) - which would make every figure here a measurement of partition handoff rather than of compute
// units.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

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
