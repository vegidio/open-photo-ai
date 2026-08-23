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
// Not every field is driven by a model yet: today only Osaka builds a profile, and it sets DynamicShapes,
// DisableMemPattern, DisableOptimizers and ExcludeEPs. The rest are reserved for per-model TensorRT and precision
// tuning that is already planned - they are deliberately kept rather than trimmed to what has a caller today, so
// treat "no setter" here as "not wired up yet", not as dead code.
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

	// GraphOptimization selects how far ONNX Runtime rewrites the graph before running it. The zero value is the
	// full pipeline, which is what every model used before profiles existed.
	GraphOptimization GraphOptimization

	// DisableOptimizers names individual graph transformers to switch off while leaving the rest of the pipeline
	// on. It is the scalpel to GraphOptimization's hammer: a model that one transformer miscompiles keeps every
	// other fusion instead of giving them all up.
	DisableOptimizers []string

	// Extra carries raw session config entries, for the settings that have no typed field here.
	Extra map[string]string
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
	for _, provider := range providers {
		if err = providerAppenders[provider](cachePath, options, p); err != nil {
			internal.Log().Debug("execution provider unavailable", "ep", provider, "err", err)
		}
	}

	return options, nil
}

// applyProfile applies the session-level settings a profile carries, as opposed to the per-provider ones.
func applyProfile(options *ort.SessionOptions, p EPProfile) error {
	if err := options.SetGraphOptimizationLevel(p.GraphOptimization.level()); err != nil {
		return errors.Wrap(err, "failed to set the graph optimization level")
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

func cudaOptions(_ EPProfile) map[string]string {
	return map[string]string{
		"device_id":                    "0",
		"do_copy_in_default_stream":    "1",
		"cudnn_conv_algo_search":       "EXHAUSTIVE",
		"cudnn_conv_use_max_workspace": "1",
		"enable_cuda_graph":            "0",
		"gpu_mem_limit":                "0",
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
		"ModelFormat":              "MLProgram",
		"MLComputeUnits":           "ALL",
		"RequireStaticInputShapes": staticShapes,
		"EnableOnSubgraphs":        "0",
		"ModelCacheDirectory":      cachePath,
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

	// TODO: Review 'trt_cuda_graph_enable' in the future; it can drastically increase the performance, but it often
	//  causes crashes when re-using the same session.
	trtOptions.Update(tensorRTOptions(cachePath, p))

	return options.AppendExecutionProviderTensorRT(trtOptions)
}

func appendCuda(_ string, options *ort.SessionOptions, p EPProfile) error {
	cudaOpts, err := ort.NewCUDAProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create CUDA EP options")
	}
	defer cudaOpts.Destroy()

	// TODO: Review 'enable_cuda_graph' in the future; it can drastically increase the performance, but it often
	//  causes crashes when re-using the same session.
	cudaOpts.Update(cudaOptions(p))

	return options.AppendExecutionProviderCUDA(cudaOpts)
}

func appendDirectML(_ string, options *ort.SessionOptions, _ EPProfile) error {
	return options.AppendExecutionProviderDirectML(0)
}

func appendCoreML(cachePath string, options *ort.SessionOptions, p EPProfile) error {
	return options.AppendExecutionProviderCoreMLV2(coreMLOptions(cachePath, p))
}

func appendOpenVINO(_ string, _ *ort.SessionOptions, _ EPProfile) error {
	return nil

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
