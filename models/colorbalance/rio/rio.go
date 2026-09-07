package rio

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/colorbalance"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to rio; the shared implementation lives in the colorbalance package.
//
// # Which weights these are
//
// Rio is Deep_White_Balance (arXiv 2004.01354), the single-task awb network, from the upstream `net_awb.pth`
// checkpoint. It is a plain 4-level U-Net - 46 tensors, and the graph's 46 initializers are those tensors unchanged,
// so this export folds nothing and re-exporting it is a shape change rather than a rebuild.
//
// # Why the canvas is a fixed square
//
// The graph rio shipped before had dynamic spatial axes, and on macOS that meant it did not run on CoreML at all.
// With RequireStaticInputShapes on - which is what a variant declaring no profile gets - the provider declined every
// one of its 52 nodes:
//
//	CoreML EP is set to only allow static input shapes. Input has a dynamic shape.
//	  Input: input, shape: {-1,3,-1,-1}
//	GetCapability: number of partitions supported by CoreML: 0
//	  number of nodes in the graph: 52  number of nodes supported by CoreML: 0
//
// So every Mac ran rio on CPU kernels. Measured on an M2 Max (ONNX Runtime 1.29, macOS 26.6, MLProgram), against the
// two shapes the old geometry produced for the common photo sizes:
//
//	                    CPU provider    CoreML
//	640x640             208.8ms         13.8ms
//	656x448             149.1ms         10.3ms
//
// Unlike paris and lyon, rio has a second way out of this: its axes are dynamic but its ops are not - Conv, Relu,
// MaxPool, ConvTranspose and Concat, nothing that needs a bounded dimension - so simply declaring DynamicShapes and
// keeping the old graph does reach CoreML, as one partition, at the numbers above. That was measured, and it is not
// what ships, for three reasons. CoreML re-specialises per shape on the flexible path, costing 34-96ms on the first
// run of each new aspect ratio. It also emits "E5RT ... ios18.max_pool: output size is too small" from inside that
// path, which is a warning today and nothing anyone should build on. And it is a macOS-only fix: TensorRT wants
// explicit optimisation profiles for a dynamic graph, which this variant has no TrtShapes to give it.
//
// # Why 656, and what it costs
//
// 656 is the ceiling the old geometry already used, so the scale the model sees is unchanged and only the padding is
// new. What a square canvas costs is measurable because the model's output is never shown: it feeds the 11-term
// polynomial fit in Process, and an 11-term fit over a few hundred thousand samples barely moves with the resolution
// it was sampled at. Scored on the final full-resolution image over 72 photos - three sources, six aspect ratios,
// four illuminant casts - against what the dynamic graph rendered:
//
//	canvas    PSNR median    p10        worst      DC median    worst
//	656       52.7 dB        45.1 dB    41.0 dB    0.09         0.62 levels
//	576       44.8 dB        41.0 dB    36.5 dB    0.18         1.78 levels
//	512       40.7 dB        36.9 dB    32.0 dB    0.36         2.70 levels
//	448       38.6 dB        34.6 dB    29.6 dB    0.46         2.74 levels
//
// A worst case of 41.0 dB and a largest DC shift of 0.62 levels is below what a viewer can see, and it is a long way
// clear of the bar paris shipped at (41.9 dB median, 31.7 dB worst, 5.61 levels). Reflection padding is also not
// interchangeable with the alternatives here: stretching to the square measures 44.5 dB median and 5.81 levels worst
// at this size, and padding without cropping the pad back out before the fit measures 47.9 dB - see Process, where
// the crop happens.
//
// The cost is inference on the pixels the padding adds. A 656 square is 1.46x the pixels of the 656x448 an ordinary
// landscape photo used to run at, so the CPU provider goes from 149ms to 222ms and a small image, which used to run
// at its own size, now runs at the canvas like everything else. On every provider that is not the CPU that trade is
// one-sided, and on the CPU it is charged against a Process call whose full-resolution polynomial apply is the
// larger half of the work.
var variant = &colorbalance.Variant{
	Codename: "rio",
	Label:    "Rio",
	Canvas:   colorbalance.Canvas{Size: 656},
	Profile:  profile,
}

// profile asks CoreML to specialise rio's fp16 graph for the one shape it accepts, and leaves fp32 on the provider
// defaults.
//
// Measured on an M2 Max at the 656x656 canvas (macOS 26.6, ONNX Runtime 1.29, CoreML MLProgram), medians of 30 rounds
// of six runs with the run order reversed on every odd round so position bias cancels. The spread column is each
// row's median against its own minimum, and it is what says the fp16 rows are separable at all - a first pass on a
// warm machine put 25% of drift on one of them and manufactured a 22% "win" for the row that really gains 3%:
//
//	fp16                     median     min     spread    vs ALL
//	ALL (default)            8.04ms     8.02ms    0.3%     +0.0%
//	ALL+FastPrediction       7.80ms     7.78ms    0.2%     -3.0%
//	ALL+lowprecaccum         8.04ms     8.02ms    0.3%     -0.0%
//	ALL+sequential           8.07ms     8.04ms    0.4%     +0.4%
//	CPUAndNeuralEngine       8.03ms     8.02ms    0.2%     -0.2%
//
//	fp32                     median     min     spread    vs ALL
//	ALL (default)           15.43ms    14.48ms    6.6%     +0.0%
//	ALL+FastPrediction      15.00ms    14.48ms    3.6%     -2.8%
//	ALL+sequential          15.18ms    14.53ms    4.4%     -1.7%
//	CPUAndNeuralEngine     110.35ms   107.81ms    2.3%   +614.9%
//
// FastPrediction is the only setting that moves this graph, and it is the first model in this codebase where it does.
// The reason is the precondition CoreMLSpecialization documents rather than luck: the option trades compile time for
// prediction latency, which is only worth paying for on a fixed-shape graph that stays resident, and rio only became
// one in the export above. The fp32 row looks like the same win and is not - both configurations bottom out at
// 14.48ms, so what differs there is variance, not compute, and fp32 is left alone.
//
// # Why rio wants the Neural Engine when paris and lyon refuse it
//
// Both of those variants pin CPUAndGPU for fp16 because ALL scatters their graphs across the Neural Engine and the
// GPU and they pay for every transition. Rio is the opposite case and it is worth stating plainly, because the shape
// of the profile is otherwise identical and the answer is inverted:
//
//	fp16 / CPUAndGPU        15.37ms    15.04ms    2.2%    +91% against ALL's minimum
//
// Rio's graph is 52 convolutional nodes, which is what the Neural Engine is built for, and it lands there whole
// rather than in pieces - so keeping it off the ANE nearly doubles the time. It costs nothing in accuracy either:
// scored on the final image against fp32 on the CPU provider, ALL measures 84.5 dB median and 64.2 dB worst against
// CPUAndGPU's 85.1 dB and 62.7 dB, a wash. Do not carry paris's or lyon's compute-unit answer here, or this one
// anywhere else.
//
// fp16 is worth having on this model in a way it is not on most: the graph's output is only ever sampled into the
// polynomial fit, so half precision is averaged away rather than shown. Against fp32 on the CPU provider it measures
// 86.5 dB median on the CPU and 84.5 dB on CoreML, with a largest DC shift of 0.058 levels.
var profile = utils.Fp16Only(utils.EPProfile{
	CoreMLSpecialization: utils.CoreMLSpecializationFastPrediction,
})

// New loads the rio session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*colorbalance.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds a rio operation at the given intensity and precision.
func Op(intensity float32, precision types.Precision) colorbalance.Op {
	return variant.Op(intensity, precision)
}
