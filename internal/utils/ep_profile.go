package utils

import (
	"fmt"
	"maps"
	"runtime"
	"slices"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// EPProfile is the per-model tuning applied on top of an execution provider's defaults.
//
// It exists because the right provider settings are a property of the model, not of the machine. CoreML's
// RequireStaticInputShapes is the clearest case: it is correct for the fixed-shape tile models, and wrong for a model
// with dynamic spatial axes, where CoreML silently pushes the varying subgraphs back to the CPU partition and reports
// nothing - the run is simply slow, with no error to explain it.
//
// The zero value reproduces the behaviour that shipped before profiles existed, which is what lets every existing
// call site keep passing no profile at all.
//
// Not every field is driven by a model yet: today Osaka sets DynamicShapes, DisableMemPattern, DisableOptimizers and
// ExcludeEPs, Athens sets CoreMLComputeUnits and - for its fp16 export only - CudaPreferNHWC, Santorini sets
// CoreMLSpecialization and ExecutionMode, and Tokyo sets CoreMLComputeUnits and ExecutionMode. The rest are reserved
// for per-model TensorRT and precision tuning that is already planned - they are deliberately kept rather than
// trimmed to what has a caller today, so treat "no setter" here as "not wired up yet", not as dead code.
type EPProfile struct {
	// DynamicShapes declares that the model's input shapes vary between runs, so providers must not be configured
	// for a fixed shape.
	DynamicShapes bool

	// Fp16 lets TensorRT run in half precision. It is opt-in: a graph already exported in fp16 gains nothing, and
	// forcing it on a model that was not validated for it silently changes the output.
	Fp16 bool

	// TrtWorkspaceBytes overrides TensorRT's workspace ceiling. Zero keeps the default.
	TrtWorkspaceBytes int64

	// TrtShapes carries the trt_profile_{min,opt,max}_shapes strings for a dynamic-shape model. TensorRT needs
	// explicit optimization profiles for those, and the right ranges are model-specific.
	TrtShapes map[string]string

	// DisableMemPattern turns off ONNX Runtime's static memory planner. The planner assumes shapes repeat between
	// runs; when they vary, it over-allocates and never returns what it reserved.
	DisableMemPattern bool

	// ExcludeEPs names providers this model must not run on. It is advisory: the next provider in the platform's
	// chain is used instead, never a silent drop to the CPU.
	ExcludeEPs []types.ExecutionProvider

	// CudaPreferNHWC runs the CUDA provider's convolutions in NHWC rather than ONNX Runtime's default NCHW.
	//
	// It is per-model, and per-precision within a model, because it is not an optimisation the runtime can pick on
	// its own merits: NHWC is the layout cuDNN's fp16 tensor-core kernels want, so an fp16 graph stops paying for
	// the transposes around every convolution, while the same graph in fp32 has no tensor-core path to reach and
	// only pays the layout conversion. Athens measures -4% end to end in fp16 and +8% in fp32 from this one flag.
	//
	// It is also not universally safe. ORT's layout transform mishandles a Conv whose weights are computed rather
	// than an initializer - a StyleGAN-style modulated convolution - and the graph then fails at Run with a channel
	// mismatch rather than at session build. That is the strongest reason this is opt-in per model: turning it on
	// globally would break models nobody re-measured.
	CudaPreferNHWC bool

	// CoreMLComputeUnits selects which of the Mac's engines CoreML may dispatch this model to. The zero value is
	// ALL, which is what every model used before profiles existed.
	CoreMLComputeUnits CoreMLComputeUnits

	// CoreMLSpecialization picks how CoreML compiles the model. The zero value is its default strategy, which is
	// what every model used before profiles existed.
	CoreMLSpecialization CoreMLSpecialization

	// GraphOptimization selects how far ONNX Runtime rewrites the graph before running it. The zero value is the
	// full pipeline, which is what every model used before profiles existed.
	GraphOptimization GraphOptimization

	// ExecutionMode selects whether ONNX Runtime may run independent branches of the graph on separate threads. The
	// zero value is parallel, which is what every model used before profiles existed.
	ExecutionMode ExecutionMode

	// DisableOptimizers names individual graph transformers to switch off while leaving the rest of the pipeline
	// on. It is the scalpel to GraphOptimization's hammer: a model that one transformer miscompiles keeps every
	// other fusion instead of giving them all up.
	DisableOptimizers []string

	// Extra carries raw session config entries, for the settings that have no typed field here.
	Extra map[string]string
}

// ResolveProfile returns the profile a variant declares for the precision being loaded, or the zero value when it
// declares none.
//
// Every model family spells "nobody has measured this graph" the same way - a nil Profile func on the variant - so
// the unwrap lives here next to EPProfile rather than being re-written in each family's variant.go.
//
// The precision is passed in because tuning does not survive the precision change: the same graph exported in fp16
// and in fp32 wants opposite answers from CudaPreferNHWC, and a profile that could not see the precision would have
// to pick the one that pessimises the other export.
func ResolveProfile(fn func(types.Precision) EPProfile, precision types.Precision) EPProfile {
	if fn == nil {
		return EPProfile{}
	}

	return fn(precision)
}

// CoreMLComputeUnits is the set of engines CoreML may dispatch a model to.
//
// It is a per-model property because the right answer follows the graph's op mix, not the machine. The Neural Engine
// is fp16-only and is built for dense convolution and matmul; a graph that is heavy in normalization, reshape and
// transpose spends more time crossing on and off it than it saves, and CoreML's planner does not work that out on its
// own - it takes the Neural Engine whenever ALL permits it.
type CoreMLComputeUnits int

const (
	// CoreMLComputeUnitsAll lets CoreML choose freely between the CPU, the GPU and the Neural Engine. It is the
	// right default: most of these graphs are convolutional, which is what the Neural Engine is good at.
	CoreMLComputeUnitsAll CoreMLComputeUnits = iota

	// CoreMLComputeUnitsCPUAndGPU keeps a model off the Neural Engine. It is for the graphs the Neural Engine
	// handles badly, where ALL costs real time in transitions.
	CoreMLComputeUnitsCPUAndGPU

	// CoreMLComputeUnitsCPUAndNeuralEngine keeps a model off the GPU, leaving it for other work.
	CoreMLComputeUnitsCPUAndNeuralEngine

	// CoreMLComputeUnitsCPUOnly runs the CoreML partition on the CPU. It is a diagnostic: it isolates whether a
	// wrong result came from the GPU's or the Neural Engine's reduced precision.
	CoreMLComputeUnitsCPUOnly
)

func (c CoreMLComputeUnits) value() string {
	switch c {
	case CoreMLComputeUnitsCPUAndGPU:
		return "CPUAndGPU"
	case CoreMLComputeUnitsCPUAndNeuralEngine:
		return "CPUAndNeuralEngine"
	case CoreMLComputeUnitsCPUOnly:
		return "CPUOnly"
	default:
		return "ALL"
	}
}

// CoreMLSpecialization is the strategy CoreML uses when it compiles a model for a device.
//
// It is a per-model property because it is a trade: the default strategy compiles once and produces a plan that is
// good across input sizes, while FastPrediction spends longer specialising for the shapes it was given. That is only
// worth paying for on a fixed-shape graph that stays resident, which is exactly the shape of these tile models - and
// on a graph whose partition CoreML was already handling well it buys nothing.
type CoreMLSpecialization int

const (
	// CoreMLSpecializationDefault leaves CoreML to its own default strategy.
	CoreMLSpecializationDefault CoreMLSpecialization = iota

	// CoreMLSpecializationFastPrediction trades compile time for prediction latency. It needs macOS 15 or newer;
	// older systems ignore it rather than failing, so setting it is safe on any Mac.
	CoreMLSpecializationFastPrediction
)

func (c CoreMLSpecialization) value() string {
	switch c {
	case CoreMLSpecializationFastPrediction:
		return "FastPrediction"
	default:
		return "Default"
	}
}

// ExecutionMode is whether ONNX Runtime may run independent branches of a graph on separate threads.
//
// It is a per-model property because the win it is there for is a property of the graph's shape: the inter-op thread
// pool only pays for itself when the graph has wide, genuinely independent branches, and it costs a thread handoff at
// every node either way. These graphs are backbones - a U-Net, a StyleGAN decoder - which are close to linear, so on
// them the handoff is all there is, and it is charged per Run rather than once.
//
// The cost is not confined to the CPU provider. Under a GPU provider every node is enqueued onto one stream, so
// inter-op threading buys nothing at all there and the scheduling still has to happen: santorini measures -4% at
// fp32 and -8% at fp16 through the CUDA provider from switching this to sequential, with a bit-identical output.
//
// What it is worth scales with the node count, since the handoff is charged per node: tokyo, at 2682 nodes against
// santorini's few hundred, measures -27% at fp32 and -18% at fp16 the same way. That is the reason to try this on
// any new graph before reaching for a provider option - it is usually the larger number, and it costs nothing.
type ExecutionMode int

const (
	// ExecutionModeParallel lets ONNX Runtime run independent branches on separate threads. It is the mode every
	// model used before profiles existed, so it stays the zero value.
	ExecutionModeParallel ExecutionMode = iota

	// ExecutionModeSequential runs one node at a time on the calling thread. It is for the graphs that have no
	// branch wide enough to pay for the inter-op pool, which is most of them.
	ExecutionModeSequential
)

func (e ExecutionMode) mode() ort.ExecutionMode {
	if e == ExecutionModeSequential {
		return ort.ExecutionModeSequential
	}

	return ort.ExecutionModeParallel
}

// GraphOptimization is how far ONNX Runtime is allowed to rewrite a graph.
type GraphOptimization int

const (
	// GraphOptimizationDefault applies every optimization, which is the right choice for a graph the runtime
	// handles correctly.
	GraphOptimizationDefault GraphOptimization = iota
	GraphOptimizationExtended
	GraphOptimizationBasic

	// GraphOptimizationDisabled runs the graph as exported. It is the last resort for a model the optimizer cannot
	// process, and it costs real speed on a transformer, where the fusions are worth a lot.
	GraphOptimizationDisabled
)

func (g GraphOptimization) level() ort.GraphOptimizationLevel {
	switch g {
	case GraphOptimizationExtended:
		return ort.GraphOptimizationLevelEnableExtended
	case GraphOptimizationBasic:
		return ort.GraphOptimizationLevelEnableBasic
	case GraphOptimizationDisabled:
		return ort.GraphOptimizationLevelDisableAll
	default:
		return ort.GraphOptimizationLevelEnableAll
	}
}

// disableOptimizersKey is the ONNX Runtime session config entry naming transformers to skip. The entries are
// semicolon-separated, and an unrecognised name is ignored rather than reported - so a typo here is silent, and the
// only way to know the setting took effect is that the model loads.
const disableOptimizersKey = "optimization.disable_specified_optimizers"

func (p EPProfile) excludes(ep types.ExecutionProvider) bool {
	return slices.Contains(p.ExcludeEPs, ep)
}

// providerAppender configures one execution provider onto a set of session options.
type providerAppender func(cachePath string, options *ort.SessionOptions, p EPProfile) error

var providerAppenders = map[types.ExecutionProvider]providerAppender{
	types.ExecutionProviderTensorRT: appendTensorRT,
	types.ExecutionProviderCUDA:     appendCuda,
	types.ExecutionProviderDirectML: appendDirectML,
	types.ExecutionProviderCoreML:   appendCoreML,
	types.ExecutionProviderOpenVINO: appendOpenVINO,
}

// autoChain is the order ExecutionProviderAuto tries on each platform, and doubles as the set of providers that
// platform supports at all: a request for one that is not listed is an error rather than a silent no-op.
//
// CPU is deliberately absent. It needs no appender, and it is what ONNX Runtime falls back to on its own once the
// providers above it decline a node.
var autoChain = map[string][]types.ExecutionProvider{
	"windows": {
		types.ExecutionProviderTensorRT,
		types.ExecutionProviderCUDA,
		types.ExecutionProviderDirectML,
		types.ExecutionProviderOpenVINO,
	},
	"linux": {
		types.ExecutionProviderTensorRT,
		types.ExecutionProviderCUDA,
		types.ExecutionProviderOpenVINO,
	},
	"darwin": {
		types.ExecutionProviderCoreML,
		types.ExecutionProviderOpenVINO,
	},
}

// resolveProviders returns the providers to append, in order, for a request on the given platform.
//
// An explicit request normally yields just that provider. When the profile excludes it, the rest of the platform's
// chain is used instead: a model that cannot run on TensorRT should fall to CUDA, not to the CPU, which would turn a
// provider mismatch into an hours-long run.
func resolveProviders(goos string, ep types.ExecutionProvider, p EPProfile) ([]types.ExecutionProvider, error) {
	chain, ok := autoChain[goos]
	if !ok {
		return nil, errors.Errorf("unsupported platform: %s", goos)
	}

	if ep == types.ExecutionProviderAuto {
		return filterExcluded(chain, p), nil
	}

	if !slices.Contains(chain, ep) {
		return nil, errors.Errorf("unsupported execution provider: %s", ep)
	}

	if !p.excludes(ep) {
		return []types.ExecutionProvider{ep}, nil
	}

	substitutes := filterExcluded(chain, p)
	internal.Log().Info("execution provider excluded for this model; substituting",
		"requested", ep, "substitutes", substitutes)

	return substitutes, nil
}

func filterExcluded(chain []types.ExecutionProvider, p EPProfile) []types.ExecutionProvider {
	out := make([]types.ExecutionProvider, 0, len(chain))

	for _, ep := range chain {
		if !p.excludes(ep) {
			out = append(out, ep)
		}
	}

	return out
}

// createOptions builds the session options for one model on one execution provider.
func createOptions(goos, cachePath string, ep types.ExecutionProvider, p EPProfile) (*ort.SessionOptions, error) {
	if _, ok := autoChain[goos]; !ok {
		return nil, errors.Errorf("unsupported platform: %s", goos)
	}

	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, errors.Wrapf(err, "failed to create %s session options", goos)
	}

	// The CPU provider is always present and takes no configuration, so there is nothing to append for it.
	if ep == types.ExecutionProviderCPU {
		return options, nil
	}

	providers, err := resolveProviders(goos, ep, p)
	if err != nil {
		options.Destroy()
		return nil, err
	}

	// A provider that declines to attach is not fatal: the ones after it, and ultimately the CPU, still run the
	// graph. That is the long-standing behaviour and the reason these errors are only logged.
	//
	// Warn rather than Debug, though. This is the line that separates "the GPU is doing the work" from "it quietly
	// fell back to CPU kernels", which is the single most common thing a user reports as "the app got slow" - and at
	// Debug it was invisible in every log anyone would actually send us. It fires at most once per provider per
	// session build, so it costs a handful of lines even on a machine where every provider declines.
	for _, provider := range providers {
		if err = providerAppenders[provider](cachePath, options, p); err != nil {
			internal.Log().Warn("execution provider declined to attach; the graph will run on the next provider "+
				"in the chain", "ep", provider, "requested_ep", ep, "err", err)
		}
	}

	return options, nil
}

// applyProfile applies the session-level settings a profile carries, as opposed to the per-provider ones.
func applyProfile(options *ort.SessionOptions, p EPProfile) error {
	if err := options.SetGraphOptimizationLevel(p.GraphOptimization.level()); err != nil {
		return errors.Wrap(err, "failed to set the graph optimization level")
	}

	if err := options.SetExecutionMode(p.ExecutionMode.mode()); err != nil {
		return errors.Wrap(err, "failed to set the execution mode")
	}

	if len(p.DisableOptimizers) > 0 {
		if err := options.AddSessionConfigEntry(disableOptimizersKey, strings.Join(p.DisableOptimizers, ";")); err != nil {
			return errors.Wrap(err, "failed to disable the specified optimizers")
		}
	}

	if p.DisableMemPattern {
		if err := options.SetMemPattern(false); err != nil {
			return errors.Wrap(err, "failed to disable the memory pattern planner")
		}
	}

	for key, value := range p.Extra {
		if err := options.AddSessionConfigEntry(key, value); err != nil {
			return errors.Wrapf(err, "failed to set session config entry %q", key)
		}
	}

	return nil
}

// region - Provider option maps
//
// Each provider's settings are built by a pure function so the mapping from profile to options can be tested without
// an ONNX Runtime present, which is the only part of this file a unit test can reach.

// CUDA graph capture - trt_cuda_graph_enable on TensorRT, enable_cuda_graph on the CUDA provider - is off in both
// option maps below, and this is why. It is not a "revisit when there is time" setting: it is measured, and both
// providers fail it, in opposite ways.
//
// On TensorRT the capture run is correct and every run after it silently returns an all-zero output. Re-measured
// against athens on 2026-09-02 (RTX 5090, driver 610.88, ONNX Runtime 1.26, both precisions): the first Run matches
// the graph-off result exactly, then 20 of 20 subsequent Runs on that session return zeros, with no error and with
// plausible timings - the replay writes nothing, which is also where the "9% faster" that makes this tempting comes
// from. In the app that surfaces as no faces found from the second detection on, or blank recovered faces, so a
// benchmark that only checks its first run will report a win and ship a broken model.
//
// On the CUDA provider it fails loudly instead: the capture run dies in cudaStreamEndCapture because cuBLAS
// initialises lazily inside the captured stream ("CUBLAS failure 1: the library was not initialized"), so the very
// first inference errors out. ORT also requires every input and output to be bound to device memory for capture,
// which this codebase's host tensors are not.
//
// Anyone revisiting this needs one thing the earlier attempt lacked: verify the SECOND Run on the SAME session
// against a CPU or graph-off result. A single-run comparison cannot see either failure.

func tensorRTOptions(cachePath string, p EPProfile) map[string]string {
	workspace := int64(4294967296)
	if p.TrtWorkspaceBytes > 0 {
		workspace = p.TrtWorkspaceBytes
	}

	fp16 := "0"
	if p.Fp16 {
		fp16 = "1"
	}

	options := map[string]string{
		"device_id":                      "0",
		"trt_max_workspace_size":         fmt.Sprintf("%d", workspace),
		"trt_fp16_enable":                fp16,
		"trt_int8_enable":                "0",
		"trt_engine_hw_compatible":       "1",
		"trt_cuda_graph_enable":          "0",
		"trt_builder_optimization_level": "5",
		"trt_engine_cache_enable":        "1",
		"trt_engine_cache_path":          cachePath,
	}

	maps.Copy(options, p.TrtShapes)
	return options
}

func cudaOptions(p EPProfile) map[string]string {
	// NCHW is ORT's default and stays the default here. NHWC is a win only where the convolutions run on fp16
	// tensor cores, and it is a measured loss on the same graph in fp32 - see EPProfile.CudaPreferNHWC.
	preferNHWC := "0"
	if p.CudaPreferNHWC {
		preferNHWC = "1"
	}

	return map[string]string{
		"cudnn_conv_algo_search":       "EXHAUSTIVE",
		"cudnn_conv_use_max_workspace": "1",
		"device_id":                    "0",
		"do_copy_in_default_stream":    "1",
		"enable_cuda_graph":            "0",
		"gpu_mem_limit":                "0",
		"prefer_nhwc":                  preferNHWC,
	}
}

func coreMLOptions(cachePath string, p EPProfile) map[string]string {
	// CoreML compiles a fixed-shape MLProgram when it may assume static inputs. For a model whose spatial axes vary
	// per run that assumption does not hold, and leaving it on makes CoreML decline the varying subgraphs silently.
	staticShapes := "1"
	if p.DynamicShapes {
		staticShapes = "0"
	}

	return map[string]string{
		"EnableOnSubgraphs":        "0",
		"MLComputeUnits":           p.CoreMLComputeUnits.value(),
		"ModelCacheDirectory":      cachePath,
		"ModelFormat":              "MLProgram",
		"RequireStaticInputShapes": staticShapes,
		"SpecializationStrategy":   p.CoreMLSpecialization.value(),
	}
}

// endregion

// region - Provider appenders

func appendTensorRT(cachePath string, options *ort.SessionOptions, p EPProfile) error {
	trtOptions, err := ort.NewTensorRTProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create TensorRT EP options")
	}
	defer trtOptions.Destroy()

	if err = trtOptions.Update(tensorRTOptions(cachePath, p)); err != nil {
		return errors.Wrap(err, "failed to apply the TensorRT EP options")
	}

	return options.AppendExecutionProviderTensorRT(trtOptions)
}

func appendCuda(_ string, options *ort.SessionOptions, p EPProfile) error {
	cudaOpts, err := ort.NewCUDAProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create CUDA EP options")
	}
	defer cudaOpts.Destroy()

	if err = cudaOpts.Update(cudaOptions(p)); err != nil {
		return errors.Wrap(err, "failed to apply the CUDA EP options")
	}

	return options.AppendExecutionProviderCUDA(cudaOpts)
}

func appendDirectML(_ string, options *ort.SessionOptions, _ EPProfile) error {
	return options.AppendExecutionProviderDirectML(0)
}

func appendCoreML(cachePath string, options *ort.SessionOptions, p EPProfile) error {
	return options.AppendExecutionProviderCoreMLV2(coreMLOptions(cachePath, p))
}

func appendOpenVINO(_ string, _ *ort.SessionOptions, _ EPProfile) error {
	// Reporting the no-op rather than returning nil. While this is stubbed out, returning nil told the caller the
	// provider had attached, so a machine resolving to OpenVINO ran entirely on CPU kernels with nothing anywhere
	// saying why. The error is not fatal - the caller logs it and moves down the chain, which is the correct
	// behaviour - it just makes the downgrade visible.
	return errors.New("the OpenVINO provider is disabled in this build")

	// TODO: Temporarily disable OpenVINO EP
	//return options.AppendExecutionProviderOpenVINO(map[string]string{
	//	"device_type":    "AUTO",
	//	"precision":      "FP32",
	//	"num_of_threads": fmt.Sprintf("%d", runtime.NumCPU()),
	//	"num_streams":    "2",
	//	"cache_dir":      cachePath,
	//})
}

// endregion

// currentPlatform is a variable, so tests can exercise the per-platform chains without cross-compiling.
var currentPlatform = runtime.GOOS
