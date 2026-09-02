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

// profileFor keeps tokyo off the Neural Engine and off the inter-op thread pool.
//
// # CoreML compute units
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
//
// # Execution mode
//
// The sequential mode is the larger of the two settings here, and it belongs to no single provider. ONNX Runtime ran
// every session in this codebase with ExecutionModeParallel, and tokyo is the worst case in the catalogue for that:
// 2682 nodes, arranged as 54 window-attention blocks in a chain, so the inter-op pool never has two independent
// nodes to hand out and all it can do is charge a thread handoff per node - 2682 of them per tile, nine tiles for
// perftest's 640x640 sample.
//
// Measured on an RTX 5090 (driver 610.88, ONNX Runtime 1.26, CUDA provider). The graph alone is one 256x256 tile,
// median of 36 runs over 3 interleaved rounds; end to end is perftest's median over 10 runs of the 640x640 sample at
// 4x, which also carries the tiling, the reflection padding and the overlap blend:
//
//	                  graph alone         end to end
//	fp32 parallel     579.3ms             5.247s
//	fp32 sequential   423.5ms (-26.9%)    3.854s (-26.6%)
//	fp16 parallel     339.4ms             3.104s
//	fp16 sequential   277.9ms (-18.1%)    2.541s (-18.1%)
//
// The output is bit-identical in both precisions, which is what makes this a setting rather than a trade. The same
// comparison on the CPU provider is 11.297s against 8.860s at fp32 and 13.483s against 12.119s at fp16, so what is
// being measured is the graph's shape and the provider only decides how much the handoff costs.
//
// TensorRT is the exception that proves it: 2.251s against 2.256s at fp32 and 959.2ms against 958.3ms at fp16, both
// within noise, because that provider swallows the graph as one fused node and leaves the inter-op pool nothing to
// schedule either way. Setting the mode unconditionally is still right - the CUDA provider is what a machine without
// TensorRT runs on, and it is where the 1.37x is.
//
// CoreML has not been re-measured with it, since the mode is set on the session rather than per provider and the
// Mac numbers above predate it. The expectation is the TensorRT one rather than the CUDA one - CoreML also takes
// the graph in a small number of partitions - so it should be a tie there, but that is reasoning, not a measurement.
//
// # What the CUDA provider options cannot do for it
//
// Nothing in cudaOptions moves this graph, and three entries would cost it, so the profile leaves them alone. Each
// was measured on top of the sequential mode above, same machine and method, against its 423.5ms / 277.9ms rows:
//
//	                                        fp32              fp16
//	cudnn_conv_algo_search=HEURISTIC        421.7ms (tie)     277.5ms (tie)
//	cudnn_conv_algo_search=DEFAULT          431.1ms (+1.8%)   292.7ms (+5.3%)
//	use_tf32=0                              443.3ms (+4.7%)   277.2ms (tie)
//	prefer_nhwc=1                           452.0ms (+6.7%)   295.8ms (+6.5%)
//	cudnn_conv_use_max_workspace=0          422.6ms (tie)     277.1ms (tie)
//	arena_extend_strategy=kSameAsRequested  422.6ms (tie)     277.3ms (tie)
//	do_copy_in_default_stream=0             422.3ms (tie)     277.9ms (tie)
//	use_ep_level_unified_stream=1           421.8ms (tie)     277.9ms (tie)
//	enable_skip_layer_norm_strict_mode=1    422.0ms (tie)     277.6ms (tie)
//
// EXHAUSTIVE and TF32, which is what cudaOptions already asks for, are the right answers for this graph - though
// only just: EXHAUSTIVE is a tie with HEURISTIC because there are 36 Convs among those 2682 nodes for it to search,
// and TF32 pays 4.7% at fp32 over the 216 Gemms and 108 MatMuls and nothing at fp16, where it does not apply.
//
// prefer_nhwc is the one to be careful about, because athens sets it and athens is also a transformer-bearing model.
// It loses here in both precisions, fp16 included, and the reason is the ratio rather than the precision: NHWC pays
// for itself by reaching cuDNN's fp16 tensor-core kernels, and 36 convolutions are not enough of this graph to repay
// the layout conversions threaded through 344 transposes. fuse_conv_bias=1 is worse than a loss - at fp32 it returns
// garbage (max|d| 2.9e+07 against the same input) and at fp16 it fails outright in the cuDNN frontend with
// HEURISTIC_QUERY_FAILED, the same way it does on santorini.
//
// Four session-level settings were tried for the same reason and land in the noise as well:
// disable_synchronize_execution_providers=1, session.use_device_allocator_for_initializers=1, turning off intra- and
// inter-op spinning, and pinning the intra-op pool to one thread are all ties. Nor is the parallel mode's cost the
// CUDA stream fan-out it looks like: use_ep_level_unified_stream=1 *without* the sequential mode measures 576.8ms at
// fp32 and 339.5ms at fp16, which is the parallel baseline unchanged. It is the scheduling, not the streams.
func profileFor(types.Precision) utils.EPProfile {
	return utils.EPProfile{
		CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU,
		ExecutionMode:      utils.ExecutionModeSequential,
	}
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
