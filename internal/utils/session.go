package utils

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// Session is an ONNX session together with the on-disk size of the files backing it, which is the proxy the model
// registry budgets against. Real device memory is larger - arenas, cuDNN workspaces and the CoreML MLProgram all sit
// on top of the weights - and none of it is queryable through these bindings, so the budget that consumes this number
// is deliberately conservative.
//
// The embedded session is promoted, so a Session is a drop-in for the *ort.DynamicAdvancedSession it wraps: Run and
// Destroy work unchanged.
type Session struct {
	*ort.DynamicAdvancedSession
	bytes int64
}

// Bytes reports the on-disk size of the files backing this session.
func (s *Session) Bytes() int64 {
	if s == nil {
		return 0
	}

	return s.bytes
}

// SessionsBytes sums Bytes over a slice of sessions, for the models that hold one session per scale factor.
func SessionsBytes(sessions []*Session) int64 {
	var total int64
	for _, session := range sessions {
		total += session.Bytes()
	}

	return total
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

func createSession(modelFile string, inputs, outputs []string, ep types.ExecutionProvider) (*Session, error) {
	internal.Log().Debug("creating session", "model_file", modelFile, "ep", ep)

	configDir, err := os.UserConfigDir()
	if err != nil {
		return nil, errors.Wrap(err, "failed to resolve user config directory")
	}

	modelsPath := filepath.Join(configDir, internal.AppName, "models")
	var options *ort.SessionOptions

	// Check the computer's OS. The default is what keeps `options` from staying nil below: without it an
	// unsupported GOOS would fall through with a nil error and blow up on the deferred Destroy.
	switch runtime.GOOS {
	case "windows":
		options, err = createWindowsOptions(modelsPath, ep)
	case "linux":
		options, err = createLinuxOptions(modelsPath, ep)
	case "darwin":
		options, err = createMacOptions(modelsPath, ep)
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
	// `<model>.data` drops the `.onnx` extension rather than appending to it, so it is derived from the stem.
	candidates := []string{
		modelPath,
		modelPath + ".data",
		modelPath + "_data",
		strings.TrimSuffix(modelPath, filepath.Ext(modelPath)) + ".data",
	}

	var total int64
	seen := make(map[string]bool, len(candidates))

	for _, candidate := range candidates {
		// A model file without an extension makes the stem-derived candidate identical to modelPath itself; counting
		// it twice would double-charge the graph.
		if seen[candidate] {
			continue
		}

		seen[candidate] = true

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
