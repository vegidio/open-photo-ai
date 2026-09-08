package saopaulo

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/colorbalance"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to saopaulo; the shared implementation lives in the colorbalance package.
//
// # What this model is, and how it differs from rio
//
// Sao Paulo is mixedillWB (Afifi et al., WACV 2022). Rio fits one global colour transform, which is the wrong answer
// for a photo lit by two sources: a window-lit room with a warm lamp, a subject in shade against a sunlit background.
// So this model does not correct an image; it blends three. It renders the photo as daylight, shade and tungsten and
// predicts a per-pixel weight map over the three, letting different regions land on different illuminants. Daylight
// is the photo itself; shade and tungsten come from Deep_White_Balance's editing network, the same architecture rio
// runs and invoked here exactly as rio invokes it.
//
// # Why it is one graph
//
// Deep_White_Balance ships net_awb.pth, net_t.pth and net_s.pth as separate files, but they are bit-identical splits
// of the multi-task net.pth: one shared encoder plus one decoder head each. Keeping the tungsten and shade heads on
// the shared encoder reproduces both files exactly - the conversion script asserts this tensor by tensor - and runs
// the encoder once instead of twice. The weight predictor bolts on after them for one Concat, so the whole
// fixed-resolution half of the pipeline is a single session:
//
//	input   [1, 3, 656, 656]   the reflection-padded square
//	output  [1, 9, 656, 656]   3 weight planes, then the shade and tungsten renderings
//
// The AWB head is dropped: it is what rio ships, and nothing here reads it. The polynomial fit and the
// full-resolution blend stay outside the graph, and the boundary falls exactly where rio's does: ONNX opset 18 has no
// Inverse or Cholesky, so an 11x11 least-squares solve cannot be expressed - but it also need not be, because the fit
// exists only to carry a canvas-resolution colour transform up to the photo's own resolution.
//
// # The upstream decoder bug, and the 35% it is worth
//
// mixedillWB's GridNet has a bug its author declined to fix (issue #4: "I will not change it to make trained models
// work as trained"). Its decoder loop reads res_blck(latent_x) where it means res_blck(x_latent), so latent_x never
// advances and only the last decoder column reaches the output block.
//
// The released checkpoints are trained with the bug, so the bug is the architecture, and what it leaves is a plain
// single-column U-Net decoder. Dropping the dead columns takes the predictor from 1,313,045 to 847,823 parameters -
// 35.4% - with bit-identical output. The encoder has no such bug and every block of it is live. Do not "fix" the bug;
// the weights were fitted around it.
//
// # The canvas, the padding and the ensemble
//
// 656 is Deep_White_Balance's own working size and the size rio already ships, so the editing half runs at exactly
// the scale it was tuned for, and one padded square serves both halves - built once, cropped once, with no second
// geometry for a rounding difference to enter through.
//
// Padding rather than squashing is worth 3.9 dB at this size. The predictor's whole job is to find regions lit
// differently, so squashing distorts exactly the spatial structure it keys on. Note that this reverses at 384, where
// squash beats pad by 1.9 dB - at a small canvas the padding is a large fraction of the square, and starving the
// image of resolution costs more than the distortion does. The preference is a property of the size, not a general
// rule.
//
// The multi-scale ensemble is worth 7.7 dB and is the single largest effect here. That is the p_128 in the
// checkpoint's name making itself felt: it was trained on 128-pixel patches, so at 656 the predictor sees structure
// well above its training scale, and the 328 and 160 members are what put that scale back in range. It is baked into
// the graph rather than done in Go, which keeps the conventional one-in-one-out shape, lets utils.ModelSpec and
// RunUnary work unchanged, and removes any chance of Go's resampling disagreeing with the reference's. It costs 1.31x
// the predictor's FLOPs on a 0.85M-parameter network sitting behind an editing network eight times its size. The
// smallest member is 160 rather than upstream's 164, because the three stride-2 stages need a multiple of 8 and we
// control the export.
//
// The canvas cannot be traded down the way rio's can, and that is the one place these two models' reasoning diverges.
// Rio's graph output is only ever sampled into an 11-term fit, which barely moves with the resolution it was sampled
// at. Half of this graph's output is not sampled into anything: the weight map is upsampled to full resolution and
// multiplies the image, so its size is a real cap on how finely the blend can follow an illuminant boundary. Dropping
// to 512 costs 3.7 dB and to 384 costs 7.6 dB.
//
// # The one node CoreML would not take
//
// The fp16 export needs a fix that the fp32 one does not, and it is the same one lyon documents. The convention here
// is fp32 in and fp32 out, so convert_float_to_float16 opens the graph with a Cast that consumes the graph input
// directly - and ONNX Runtime's CoreML Cast builder declines a Cast with no producer, saying so in as many words in
// the verbose log ("Cast has no preceding nodes"). That stranded a 1x3x656x656 conversion on the CPU partition and
// left the graph at 510 of 511 nodes.
//
// The conversion script now splices a Clip(input, 0, 1) ahead of that Cast, which takes it to 511 of 511 with every
// node on CoreML. It has to be done on the converted graph rather than in the exported module - a clamp in forward()
// lands *after* the boundary Cast, because the converter always inserts that Cast at the input, so it fixes nothing -
// and it is a real node rather than one inserted to fool the partitioner: [0,1] is this model's input contract, the
// renderings are clipped on the way out for the same reason, and the rendered image is byte-identical with and
// without it. The fp32 graph has no such Cast and is a clean 483 of 483.
//
// Anyone re-measuring anything below must confirm both graphs are still one CoreML partition first, and must do it
// on the runtime the app ships - internal/artifacts.go pins 1.26.0. This is not pedantry: on 1.29 the stray Cast is
// absorbed and the graph reads as fully fused whether or not the Clip is there, so a check run on a newer runtime
// silently passes a graph that the shipping one splits.
//
// # How the two precisions compare
//
// On an M2 Max fp16 is the slower of the two on CoreML (123ms against fp32's 110ms), which is unusual, and it is not
// a partitioning failure - both graphs are one fused CoreML node. Nor is it something a setting fixes: the provider
// options below, and the sequential execution mode against the parallel one the app defaults to, were all measured
// and none of them separate. fp16's problem is consistency rather than speed: its minimum is the faster of the two
// but it carries an 11-18% spread where fp32 sits at 1-3%, so its median lands behind.
//
// It ships regardless, and not grudgingly: it is half the download at 16.7 MB against 33.2 MB, it is what the
// interface's SD tier selects, and against fp32 on the same provider it measures 66.8 dB - a largest single-channel
// error of one 8-bit level.
//
// Both precisions are bit-identical between the CPU and CoreML providers, so which provider a machine picks cannot
// change what the user sees.
//
// Against the upstream Python pipeline at its own defaults the Go implementation measures 43.5 dB on the sample -
// about one 8-bit level on average, against the 5.6 levels the correction itself moves. The gap is the deliberate
// divergences rather than an error: a padded 656 square where upstream squashes to 384, one polynomial fit at the
// canvas where upstream fits twice, and none of upstream's uint8 round-trip between the two.
//
// # What is not implemented
//
// Upstream's default post-processing refines each weight channel with a fast bilateral solver (Barron and Poole)
// guided by the image, then renormalises. It is optional there (--post-process), it cannot be expressed in ONNX, and
// a sparse-CG bilateral solver at full photo resolution is out of proportion to what it buys on a weight map that is
// already smooth. Skipping it costs a little edge snapping at illuminant boundaries. A guided filter would recover
// most of that for a fraction of the work if it ever proves worth having.
var variant = &colorbalance.Variant{
	Codename: "saopaulo",
	Label:    "São Paulo",
	Canvas:   colorbalance.Canvas{Size: 656},
	Mixed:    &colorbalance.MixedSpec{Settings: 3},

	// No profile, and that is a measured result rather than an omission. Rio's FastPrediction was measured on the
	// editing network alone; this graph is that network plus a weight predictor plus two internal resamples, a
	// different op mix, which is exactly why rio's own comment says a profile does not travel between models just
	// because both correct colour.
	//
	// Swept on an M2 Max at the 656 canvas, nothing beats the CoreML defaults: FastPrediction's 3.3% on fp16 is the
	// only row that even looks like a win and it sits inside that row's own 6-7% run-to-run spread. Execution mode is
	// also a wash, which is worth recording because the app defaults to the parallel mode rather than ONNX Runtime's
	// sequential one: on a graph that is a single fused CoreML node the inter-op pool has nothing to schedule either
	// way.
	//
	// The one unambiguous result is that keeping this graph off the GPU is very expensive - CPUAndNeuralEngine costs
	// +62% at fp16 and +732% at fp32 - and that is the same answer rio reaches from the opposite direction. Rio is 52
	// convolutional nodes that land on the Neural Engine whole; this is 511 nodes including resamples and a softmax,
	// and the ANE cannot take all of it.
	Profile: nil,
}

// New loads the saopaulo session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorbalance.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a saopaulo operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) colorbalance.Op {
	return variant.Op(intensity, precision)
}
