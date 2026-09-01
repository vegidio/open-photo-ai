package tokyo

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to tokyo; the shared implementation lives in the upscale package.
var variant = &upscale.Variant{
	Label:        "Tokyo",
	Codename:     "tokyo",
	ScaleBuckets: upscale.DefaultScaleBuckets,
	Profile:      profileFor,
}

// profileFor keeps tokyo off the Neural Engine.
//
// Measured on an M2 Max (macOS 26.6, ONNX Runtime 1.29) over one 256x256 tile, median of 9 runs:
//
//	                       fp32                 fp16
//	MLComputeUnits=ALL     1718ms               2255ms
//	CPUAndGPU              1709ms (unchanged)   1351ms (1.67x)
//	CPUAndNeuralEngine     -                    30160ms (22x slower)
//
// fp32 is a wash between ALL and CPUAndGPU, so on speed alone this setting would only be worth it for fp16. The
// reason it is set for both is that the Neural Engine cannot compile this graph at all. SwinIR is window attention:
// 452 reshapes, 344 transposes and 110 normalizations against 36 convolutions, which is the opposite of what a unit
// built for dense convolution wants. Asking for it produces `_ANECompiler : ANECCompile() FAILED` - 54 of them in one
// session build, one per transformer block - and the failures are not free: the CPUAndNeuralEngine session above took
// 1067 seconds to build before running 22x slower than the GPU. SpecializationStrategy=FastPrediction, which also
// routes through that compiler, wedged the process outright at 11.5 GB of resident memory.
//
// So CPUAndGPU is chosen for what it excludes as much as for what it measures: it is the one setting that cannot
// reach that compiler. ALL is not safe here merely because it is usually the right default - it lets CoreML try the
// Neural Engine, which is exactly the path that fails.
//
// How far this carries to other Macs: the cause is the graph's op mix and an ANE compiler limitation, both of which
// are the same everywhere, so the direction should hold on any Apple Silicon Mac; on Intel there is no Neural Engine
// and the setting is a no-op. The 1.67x is this machine's number - a smaller GPU narrows it.
func profileFor() utils.EPProfile {
	return utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU}
}

// New loads the tokyo sessions for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*upscale.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a tokyo operation at the given scale and precision.
func Op(scale float64, precision types.Precision) upscale.Op {
	return variant.Op(scale, precision)
}
