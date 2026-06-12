package utils

import (
	"os"
	"path/filepath"
	"runtime"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

func CreateSession(modelFile string, inputs, outputs []string, ep types.ExecutionProvider) (*ort.DynamicAdvancedSession, error) {
	internal.Log().Debug("creating session", "model_file", modelFile, "ep", ep)

	configDir, err := os.UserConfigDir()
	cachePath := filepath.Join(configDir, internal.AppName, "models")
	var options *ort.SessionOptions

	// Check the computer's OS
	switch runtime.GOOS {
	case "windows":
		options, err = createWindowsOptions(cachePath, ep)
	case "linux":
		options, err = createLinuxOptions(cachePath, ep)
	case "darwin":
		options, err = createMacOptions(cachePath, ep)
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

	modelPath := filepath.Join(configDir, internal.AppName, "models", modelFile)
	session, err := ort.NewDynamicAdvancedSession(modelPath, inputs, outputs, options)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create session")
	}

	return session, nil
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
		return nil, errors.Errorf("unsupported execution provider: %x", ep)
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
		return nil, errors.Errorf("unsupported execution provider: %x", ep)
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
		return nil, errors.Errorf("unsupported execution provider: %x", ep)
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
