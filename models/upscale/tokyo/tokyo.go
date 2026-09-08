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
// CoreML has now been measured rather than reasoned about, and the reasoning above was only half right: fp32 is the
// predicted tie, but fp16 is a real win. Measured out of tree on the M2 Max above, isolating Run over one 256x256
// tile, 8 blocks of 3 runs across four interleaved rounds:
//
//	                    fp32                fp16
//	Parallel            1903.2ms            2318.5ms
//	Sequential          1907.9ms (+0.2%)    2237.8ms (-3.5%)
//
// So the mode costs this graph nothing on a Mac and gains it something at fp16, while being worth 1.37x on CUDA.
// Output is bit-identical between the two modes in both precisions there as well.
//
// Read those two columns as ratios and not as absolutes: they come from different sweeps, and this graph is slow
// enough that the Mac's thermal state moves it more than the mode does. The same fp16 parallel configuration
// measured 1619ms on a cold machine, 4780ms on a hot one and 2319ms after a three-minute pause. For the same reason,
// do not try to reproduce this end to end through perftest with one mode per process - that put one configuration at
// 12.2s and then 31.4s two rounds later. The in-process harness interleaves the modes so drift moves both rows.
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
//
// # What the TensorRT provider options cannot do for it either
//
// TrtOptions is empty for the same reason CudaOptions is, and the answer here is even flatter: TensorRT swallows all
// 2682 nodes into a single engine - the cached file ends `_0_0`, one subgraph - so there is no partitioning left to
// tune and every remaining option is a builder setting that lands on the same fused engine.
//
// Measured on an RTX 5090 (driver 610.88, ONNX Runtime 1.26, TensorRT 10, sm_120), one 256x256 tile, sequential mode,
// median of 18 runs over 6 interleaved rounds, each configuration given its own engine cache directory:
//
//	                                              fp32              fp16
//	trt_max_workspace_size=1 GiB                  -0.6%             +0.1%
//	trt_max_workspace_size=8 GiB                  +0.1%             +2.5%
//	trt_max_workspace_size=16 GiB                 -0.3%             +2.5%
//	trt_auxiliary_streams=0                       +0.2%             +6.2%
//	trt_auxiliary_streams=1                       +0.1%             -0.0%
//	trt_auxiliary_streams=4                       +1.0%             +0.0%
//	trt_builder_optimization_level=3              +0.2%             +0.3%
//	trt_builder_optimization_level=4              -0.1%             +1.3%
//	trt_tactic_sources=+CUBLAS,+CUBLAS_LT         +0.2%             +0.9%
//	trt_tactic_sources=-CUDNN                     +0.6%             +0.2%
//	trt_sparsity_enable=1                         +0.2%             +1.5%
//	trt_context_memory_sharing_enable=1           -1.0%             +1.9%
//	trt_layer_norm_fp32_fallback=1                -1.6%             +6.2%
//
// Every row of that table is noise, including the two that look like findings, and the way to see it is the last row.
// trt_layer_norm_fp32_fallback only does anything when TensorRT has been allowed to pick half precision, which
// neither column does, and both of its engines came out bit-identical to their baseline - yet it measures -1.6% in
// one column and +6.2% in the other. A setting with no effect cannot be worth 6%, so the spread belongs to something
// other than the setting.
//
// It belongs to the builder. Six sessions built from the SAME configuration - no option changed, one engine cache
// directory each - spread from -2.5% to +1.2% at fp16, and five of the six produced output differing from the first
// by 6.3e-3, so TensorRT is not choosing the same tactics twice. That is the floor: one build per configuration
// cannot resolve anything below about +/-3%, and a sweep that reports otherwise is reporting its own builds. Run
// against it, trt_auxiliary_streams=0 - the largest non-precision number above - averages +1.4% over three
// independent builds against a baseline that itself spans 1.3%, which is nothing.
//
// Two traps are worth writing down for whoever sweeps the next graph, because both silently produce a clean table of
// wrong numbers. TensorRT's cache file name carries only the graph hash and the precision flags, so configurations
// sharing one engine cache directory hand each other the first one's engine and every option reads as a no-op. And
// the non-determinism above means a single build per configuration is a sample of the builder, not a measurement of
// the option.
//
// # The one setting that does move it, and why it is not set
//
// trt_fp16_enable is worth 2.66x on the fp32 export, and it is exactly the fp16 export. Three builds each, fp32
// export on the provider defaults as the reference:
//
//	                                    median       max|d|      mean|d|
//	fp32 export, defaults               107.2ms      -            -
//	fp32 export, trt_fp16_enable=1       40.3ms      6.6e-3       4.5e-4
//	fp16 export, defaults                41.0ms      7.0e-3       4.4e-4
//
// The two half-precision rows agree on speed to 1.7% and on both deviation figures to two significant figures, which
// is the point: turning the flag on for the fp32 export does not produce a faster fp32 model, it produces the fp16
// model under the fp32 model's name. On TensorRT alone, since CUDA, CoreML and the CPU would keep running the graph
// as exported - so the same operation would carry a different precision depending on which provider the machine
// resolved to.
//
// So this is the TensorRT twin of the CoreML ModelFormat=NeuralNetwork trap described in coreMLOptions, and it is
// declined for the same reason: it is the largest number a provider sweep will find, and shipping it would hand a
// user who asked for fp32 a precision downgrade they never chose. The honest way to take the 2.66x is to select the
// fp16 model, which is what EPProfile.Fp16 is opt-in for.
//
// trt_bf16_enable is the same trade taken badly and needs no such care: -55.2% on the fp32 export at max|d| 3.3e-2 -
// slower than fp16 and five times the deviation - and +23.6% on the fp16 export, where it is a plain loss.
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
