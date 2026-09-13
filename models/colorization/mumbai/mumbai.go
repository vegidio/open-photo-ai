package mumbai

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/colorization"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to mumbai; the shared implementation lives in the colorization package.
var variant = &colorization.Variant{
	Codename: "mumbai",
	Label:    "Mumbai",
	Spec:     colorization.DDColor,
	Profile:  profile,
}

// profile keeps mumbai's fp16 export off the Neural Engine. On the graph as it is exported today that is worth 34.9%;
// on the re-exported graph described below it is the difference between a correct image and a broken one.
//
// # Why the Neural Engine is wrong for it
//
// DDColor is a ConvNeXt encoder feeding a transformer colour decoder, and the op mix says so: 155 Gemms, 147
// transposes and 64 normalizations against 51 convolutions. That is the shape tokyo documents - the opposite of what a
// unit built for dense convolution wants - and here the Neural Engine does not merely lose, it returns garbage. Run
// with ALL against an fp32 reference the rebuilt fp16 graph deviates by 334 in Lab ab units, on planes whose whole
// range is about +/-64, and it takes 923ms against the 95ms it takes on the GPU. CPUAndGPU is the one setting that
// cannot reach that compiler.
//
// The published graph hides this rather than avoiding it. It has five nodes the CoreML EP declines - four Pads in
// PixelShuffle_ICNR's blur, which MLProgram only supports in `constant` and `reflect` modes, and the colour decoder's
// Einsum, which the EP has no builder for at all - so it partitions into six CoreML subgraphs around them and the
// Neural Engine never sees the whole model. That is also why fp16 is currently the SLOWER export: 246ms against
// fp32's 185ms, because one of those CPU nodes is the bqc,bchw->bqhw product against a [1,256,512,512] feature map,
// which crosses the partition boundary as 268 MB per run and is then multiplied out on the CPU.
//
// Rewriting those five nodes into ops CoreML does support - edge padding as Slice+Concat of the border row and
// column, the Einsum as Reshape+MatMul+Reshape - collapses the graph to one partition and changes nothing
// numerically: 3.1e-5 in ab against the published graph, at most one 8-bit LSB in the rendered image. Measured
// against the published exports in one sweep, that is 185.2ms -> 101.3ms at fp32 (-45.3%) and 246.4ms -> 95.1ms at
// fp16 (-61.4%), and it also makes fp16 the more accurate export rather than the less: 0.87 in ab against the fp32
// reference, where the published fp16 measures 5.72 because part of it is already on the Neural Engine.
//
// The two changes are independent, which is what makes the ordering safe: this profile is a 34.9% win on the
// published six-partition graph (298.8ms -> 194.5ms) as well as a correctness requirement on the one-partition one,
// so it does not have to land in the same release as the weights.
//
// # Why fp32 gets nothing
//
// An MLProgram at fp32 cannot reach the Neural Engine at all, so ALL and CPUAndGPU are the same session: they measure
// within 0.4% of each other on the rebuilt graph with bit-identical output. Fp16Only is what keeps that from being
// stated as a setting.
//
// # What else was measured and did not earn a setting
//
// On the rebuilt fp16 graph, against CPUAndGPU: ExecutionModeSequential -0.5%, SpecializationStrategy=FastPrediction
// -0.6%, and AllowLowPrecisionAccumulationOnGPU=1 - which has no field on EPProfile and was reached through a
// temporary overlay - -0.7%. All three are bit-identical and all three are inside the noise floor, so none of them is
// a reason to add the CoreML counterpart of TrtOptions.
//
// Nor does any of this help the cold start, which is the graph's real remaining cost: CoreML spends about five
// minutes compiling the MLProgram on a first run whether it is producing one program or six, and the partition count
// does not move it.
var profile = utils.Fp16Only(utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU})

// New loads the mumbai session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorization.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a mumbai operation at the given precision.
func Op(precision types.Precision) colorization.Op {
	return variant.Op(precision)
}
