package utils

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

// Session is an ONNX session together with the on-disk size of the files backing it, which is the proxy the model
// registry budgets against. Real device memory is larger - arenas, cuDNN workspaces and the CoreML MLProgram all sit
// on top of the weights - and none of it is queryable through these bindings, so the budget that consumes this number
// is deliberately conservative.
//
// The embedded session is promoted, so a Session is a drop-in for the *ort.DynamicAdvancedSession it wraps: Run and
// Destroy work unchanged.
//
// Models embed a *Session rather than holding it in a named field, which promotes ResidentBytes and Destroy and so
// satisfies types.Measurable and types.Destroyable without any per-model code. That is what stops a new model from
// silently being charged the default size because someone forgot to write the method.
type Session struct {
	*ort.DynamicAdvancedSession
	bytes int64
}

// ResidentBytes implements types.Measurable, reporting the on-disk size of the files behind this session.
func (s *Session) ResidentBytes() int64 {
	if s == nil {
		return 0
	}

	return s.bytes
}

// Destroy releases the native resources behind the session. It implements types.Destroyable.
//
// It exists to drop the error that the ONNX binding's own Destroy returns, which types.Destroyable does not have: a
// model that fails to free is unreachable either way, and there is nothing a caller could do about it. Without this
// method the promoted one would be the wrong shape and no model would satisfy the interface.
func (s *Session) Destroy() {
	if s == nil || s.DynamicAdvancedSession == nil {
		return
	}

	if err := s.DynamicAdvancedSession.Destroy(); err != nil {
		internal.Log().Warn("failed to destroy ONNX session", "err", err)
	}
}

// Sessions is the set of sessions behind one model, for the upscalers that hold one per scale factor. It reports and
// releases them as a unit, so those models embed it exactly the way the single-session models embed a *Session.
type Sessions []*Session

// ResidentBytes sums the on-disk size of every session in the set. It implements types.Measurable.
func (s Sessions) ResidentBytes() int64 {
	var total int64
	for _, session := range s {
		total += session.ResidentBytes()
	}

	return total
}

// Destroy releases every session in the set. It implements types.Destroyable.
func (s Sessions) Destroy() {
	for _, session := range s {
		session.Destroy()
	}
}

// CreateSession builds an ONNX session for the given model file and execution provider.
//
// Every failure it returns - the provider options, the session options and the model load alike - is marked with
// internal.ErrCreateSession, so callers can tell "this session couldn't be built" apart from the failures around it,
// such as a model that couldn't be downloaded. That's what lets AcquireModel decide a retry on the CPU is worth
// attempting.
func CreateSession(modelFile string, inputs, outputs []string, ep types.ExecutionProvider) (*Session, error) {
	session, err := createSession(modelFile, inputs, outputs, ep)
	if err != nil {
		return nil, errors.Mark(err, internal.ErrCreateSession)
	}

	return session, nil
}

// LoadSingleSession downloads and opens the one session behind a model whose ID is `<prefix>_<variant>_<precision>`,
// e.g. `dn_stockholm_fp16`. It covers the fixed-shape families that have no scale matrix - denoise and sharpen - which
// otherwise carry a byte-identical copy of this function each.
func LoadSingleSession(
	ctx context.Context,
	prefix, variant string,
	precision types.Precision,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Session, error) {
	modelId := fmt.Sprintf("%s_%s_%s", prefix, variant, precision)
	modelFile := modelId + ".onnx"

	if err := deps.Install(ctx, deps.ModelDependency(modelId), onProgress); err != nil {
		return nil, errors.Wrapf(err, "failed to prepare %s model", variant)
	}

	internal.Log().Debug("loading model session", "model_id", modelId)

	session, err := CreateSession(modelFile, []string{"input"}, []string{"output"}, ep)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to create %s session", variant)
	}

	internal.Log().Debug("model session ready", "model_id", modelId)

	return session, nil
}

// FormatModelName builds the display name a model family shows in the UI, e.g. "Denoise (FP16)".
func FormatModelName(label string, precision types.Precision) string {
	return fmt.Sprintf("%s (%s)", label, cases.Upper(language.English).String(string(precision)))
}

func createSession(modelFile string, inputs, outputs []string, ep types.ExecutionProvider) (*Session, error) {
	internal.Log().Debug("creating session", "model_file", modelFile, "ep", ep)

	modelsPath, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return nil, errors.Wrap(err, "failed to resolve the models directory")
	}

	// The execution providers compile what they cache - a TensorRT engine, a CoreML MLProgram - from this model's
	// weights, so the cache gets a directory of its own per model. Sharing one directory with the models, as this used
	// to, made the cache impossible to invalidate: TensorRT drops its .engine and .profile files loose beside the
	// models and names them after the graph inside the file rather than the file itself, so nothing said whose cache
	// was whose. With a directory per model, replacing a model is enough to drop exactly its cache.
	//
	// The directory comes from EngineCacheFor, which is also what deps.Install clears when it replaces the weights.
	stem := strings.TrimSuffix(modelFile, filepath.Ext(modelFile))
	cachePath, err := fs.MkUserConfigDir(internal.AppName, strings.Split(internal.EngineCacheFor(stem), "/")...)
	if err != nil {
		return nil, errors.Wrap(err, "failed to resolve the engine cache directory")
	}

	var options *ort.SessionOptions

	// The default case is what keeps `options` from staying nil below: without it an unsupported GOOS would fall
	// through with a nil error and blow up on the deferred Destroy.
	switch runtime.GOOS {
	case "windows":
		options, err = createWindowsOptions(cachePath, ep)
	case "linux":
		options, err = createLinuxOptions(cachePath, ep)
	case "darwin":
		options, err = createMacOptions(cachePath, ep)
	default:
		return nil, errors.Errorf("unsupported platform: %s", runtime.GOOS)
	}

	if err != nil {
		return nil, errors.Wrap(err, "failed to create session options")
	}
	defer options.Destroy()

	// Extra session options
	if err = options.SetGraphOptimizationLevel(ort.GraphOptimizationLevelEnableAll); err != nil {
		return nil, errors.Wrap(err, "failed to set graph optimization level")
	}

	if err = options.SetExecutionMode(ort.ExecutionModeParallel); err != nil {
		return nil, errors.Wrap(err, "failed to set execution mode")
	}

	modelPath := filepath.Join(modelsPath, modelFile)
	session, err := ort.NewDynamicAdvancedSession(modelPath, inputs, outputs, options)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create session")
	}

	return &Session{DynamicAdvancedSession: session, bytes: modelFileBytes(modelPath)}, nil
}

// modelFileBytes reports the total size of the files backing the model at modelPath: the graph itself plus any
// external-data blob stored beside it.
//
// ONNX keeps the weights of a model larger than the 2 GB protobuf limit in a separate file, named by a `location`
// field inside the graph. Reading that field would mean parsing the protobuf, so this probes the three naming
// conventions in use instead (`<model>.onnx.data`, `<model>.onnx_data`, `<model>.data`). That matters by three orders
// of magnitude for the diffusion upscaler: `up_osaka_fp16.onnx` is 7.7 MB and `up_osaka_fp16.onnx.data` is 6.8 GB, so
// charging the budget for the graph alone would make a 7 GB model look free.
//
// Probing exact names rather than globbing the directory matters for two reasons. The models directory doubles as the
// TensorRT engine cache and the CoreML model cache, so a glob's cost grows with every compiled kernel - and this runs
// on every session build, once per scale for the upscalers and twice on the CPU-fallback path. It also keeps a model
// that merely shares a stem out of the total by construction: `up_osaka_vae_decoder_fp16.onnx` is its own model, not
// part of `up_osaka_fp16`.
//
// A file that can't be stat-ed contributes 0: an under-count degrades the budget, whereas failing here would fail the
// session build.
func modelFileBytes(modelPath string) int64 {
	candidates := []string{modelPath, modelPath + ".data", modelPath + "_data"}

	// `<model>.data` drops the `.onnx` extension rather than appending to it, so it is derived from the stem - and is
	// only a distinct candidate when there is an extension to drop. Without one the stem is the whole path, making it
	// a duplicate of `<model>.data` above, which would charge that blob twice.
	if ext := filepath.Ext(modelPath); ext != "" {
		candidates = append(candidates, strings.TrimSuffix(modelPath, ext)+".data")
	}

	var total int64

	for _, candidate := range candidates {
		info, err := os.Stat(candidate)
		if err != nil || info.IsDir() {
			continue
		}

		total += info.Size()
	}

	return total
}

// region - OS specific options

func createWindowsOptions(cachePath string, ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Windows session options")
	}

	switch ep {
	case types.ExecutionProviderCPU:
		return options, nil
	case types.ExecutionProviderTensorRT:
		_ = getTensorRTEP(cachePath, options)
	case types.ExecutionProviderCUDA:
		_ = getCudaEP(cachePath, options)
	case types.ExecutionProviderDirectML:
		_ = getDirectMLEP(cachePath, options)
	case types.ExecutionProviderOpenVINO:
		_ = getOpenVINOEP(cachePath, options)
	case types.ExecutionProviderAuto:
		_ = getTensorRTEP(cachePath, options)
		_ = getCudaEP(cachePath, options)
		_ = getDirectMLEP(cachePath, options)
		_ = getOpenVINOEP(cachePath, options)
	default:
		options.Destroy()
		return nil, errors.Errorf("unsupported execution provider: %s", ep)
	}

	return options, nil
}

func createLinuxOptions(cachePath string, ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Linux session options")
	}

	switch ep {
	case types.ExecutionProviderCPU:
		return options, nil
	case types.ExecutionProviderTensorRT:
		_ = getTensorRTEP(cachePath, options)
	case types.ExecutionProviderCUDA:
		_ = getCudaEP(cachePath, options)
	case types.ExecutionProviderOpenVINO:
		_ = getOpenVINOEP(cachePath, options)
	case types.ExecutionProviderAuto:
		_ = getTensorRTEP(cachePath, options)
		_ = getCudaEP(cachePath, options)
		_ = getOpenVINOEP(cachePath, options)
	default:
		options.Destroy()
		return nil, errors.Errorf("unsupported execution provider: %s", ep)
	}

	return options, nil
}

func createMacOptions(cachePath string, ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, errors.Wrap(err, "failed to create macOS session options")
	}

	switch ep {
	case types.ExecutionProviderCPU:
		return options, nil
	case types.ExecutionProviderCoreML:
		_ = getCoreMLEP(cachePath, options)
	case types.ExecutionProviderOpenVINO:
		_ = getOpenVINOEP(cachePath, options)
	case types.ExecutionProviderAuto:
		_ = getCoreMLEP(cachePath, options)
		_ = getOpenVINOEP(cachePath, options)
	default:
		options.Destroy()
		return nil, errors.Errorf("unsupported execution provider: %s", ep)
	}

	return options, nil
}

// endregion

// region - Execution provider configuration

func getTensorRTEP(cachePath string, options *ort.SessionOptions) error {
	trtOptions, err := ort.NewTensorRTProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create TensorRT EP options")
	}
	defer trtOptions.Destroy()

	// TODO: Review 'trt_cuda_graph_enable' in the future; it can drastically increase the performance, but it often
	//  causes crashes when re-using the same session.
	trtOptions.Update(map[string]string{
		"device_id":                      "0",
		"trt_max_workspace_size":         "4294967296",
		"trt_fp16_enable":                "0",
		"trt_int8_enable":                "0",
		"trt_engine_hw_compatible":       "1",
		"trt_cuda_graph_enable":          "0",
		"trt_builder_optimization_level": "5",
		"trt_engine_cache_enable":        "1",
		"trt_engine_cache_path":          cachePath,
	})

	return options.AppendExecutionProviderTensorRT(trtOptions)
}

func getCudaEP(_ string, options *ort.SessionOptions) error {
	cudaOptions, err := ort.NewCUDAProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create CUDA EP options")
	}
	defer cudaOptions.Destroy()

	// TODO: Review 'enable_cuda_graph' in the future; it can drastically increase the performance, but it often
	//  causes crashes when re-using the same session.
	cudaOptions.Update(map[string]string{
		"device_id":                    "0",
		"do_copy_in_default_stream":    "1",
		"cudnn_conv_algo_search":       "EXHAUSTIVE",
		"cudnn_conv_use_max_workspace": "1",
		"enable_cuda_graph":            "0",
		"gpu_mem_limit":                "0",
	})

	return options.AppendExecutionProviderCUDA(cudaOptions)
}

func getDirectMLEP(_ string, options *ort.SessionOptions) error {
	return options.AppendExecutionProviderDirectML(0)
}

func getCoreMLEP(cachePath string, options *ort.SessionOptions) error {
	return options.AppendExecutionProviderCoreMLV2(map[string]string{
		"ModelFormat":              "MLProgram",
		"MLComputeUnits":           "ALL",
		"RequireStaticInputShapes": "1",
		"EnableOnSubgraphs":        "0",
		"ModelCacheDirectory":      cachePath,
	})
}

func getOpenVINOEP(cachePath string, options *ort.SessionOptions) error {
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
