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
	Spec:     colorization.AbGraph,
	Profile:  profile,
}

// profile keeps mumbai's fp16 export off the Neural Engine. On the graph as exported today that is worth 34.9%
// (298.8ms -> 194.5ms); on the re-exported graph described below it is the difference between a correct image and a
// broken one - run with ALL against an fp32 reference the rebuilt fp16 graph deviates by 334 in Lab ab units, on
// planes whose whole range is about +/-64, and takes 923ms against the 95ms it takes on the GPU. CPUAndGPU is the one
// setting that cannot reach that compiler.
//
// DDColor is a ConvNeXt encoder feeding a transformer colour decoder, and the op mix says so: 155 Gemms, 147
// transposes and 64 normalizations against 51 convolutions - the shape tokyo documents as the opposite of what a unit
// built for dense convolution wants.
//
// The published graph hides this rather than avoiding it. Five nodes the CoreML EP declines partition it into six
// CoreML subgraphs, so the Neural Engine never sees the whole model. That is also why fp16 is currently the SLOWER
// export - 246ms against fp32's 185ms - because one of those CPU nodes is the bqc,bchw->bqhw product against a
// [1,256,512,512] feature map, which crosses the partition boundary as 268 MB per run.
//
// Export note, to collapse it to one partition: the four Pads in PixelShuffle_ICNR's blur (MLProgram supports only
// `constant` and `reflect`) rewritten as a Slice+Concat of the border row and column, and the colour decoder's Einsum,
// which the EP has no builder for, as Reshape+MatMul+Reshape. Numerically a no-op at 3.1e-5 in ab, worth -45.3% at
// fp32 and -61.4% at fp16, and it makes fp16 the more accurate export rather than the less. It is independent of this
// profile, so the two do not have to land in the same release.
//
// Sequential, FastPrediction and AllowLowPrecisionAccumulationOnGPU all measured within 0.7% and bit-identical.
//
// None of this helps the cold start, which is the graph's real remaining cost: CoreML spends about five minutes
// compiling the MLProgram on a first run whether it is producing one program or six.
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
