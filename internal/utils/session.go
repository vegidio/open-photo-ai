package utils

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

func CreateSession(appName, modelName string) (*ort.DynamicAdvancedSession, error) {
	configDir, err := os.UserConfigDir()
	cachePath := filepath.Join(configDir, appName, "models")
	var options *ort.SessionOptions

	// Check the computer's OS
	switch runtime.GOOS {
	case "windows":
		options, err = createWindowsOptions(cachePath, types.ExecutionProviderBest)
	case "linux":
		options, err = createLinuxOptions(cachePath, types.ExecutionProviderBest)
	case "darwin":
		options, err = createMacOptions(cachePath, types.ExecutionProviderBest)
	}

	if err != nil {
		return nil, err
	}
	defer options.Destroy()

	modelPath := filepath.Join(configDir, appName, "models", modelName)
	session, err := ort.NewDynamicAdvancedSession(modelPath, []string{"input"}, []string{"output"}, options)
	if err != nil {
		return nil, err
	}

	return session, nil
}

// region - OS specific options

func createWindowsOptions(cachePath string, ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, err
	}

	options.SetGraphOptimizationLevel(ort.GraphOptimizationLevelEnableAll)
	options.SetExecutionMode(ort.ExecutionModeParallel)

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
	case types.ExecutionProviderBest:
		_ = getTensorRTEP(cachePath, options)
		_ = getCudaEP(cachePath, options)
		_ = getDirectMLEP(cachePath, options)
		_ = getOpenVINOEP(cachePath, options)
	default:
		return nil, fmt.Errorf("unsupported execution provider: %x", ep)
	}

	return options, nil
}

func createLinuxOptions(cachePath string, ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, err
	}

	options.SetGraphOptimizationLevel(ort.GraphOptimizationLevelEnableAll)
	options.SetExecutionMode(ort.ExecutionModeParallel)

	switch ep {
	case types.ExecutionProviderCPU:
		return options, nil
	case types.ExecutionProviderTensorRT:
		_ = getTensorRTEP(cachePath, options)
	case types.ExecutionProviderCUDA:
		_ = getCudaEP(cachePath, options)
	case types.ExecutionProviderOpenVINO:
		_ = getOpenVINOEP(cachePath, options)
	case types.ExecutionProviderBest:
		_ = getTensorRTEP(cachePath, options)
		_ = getCudaEP(cachePath, options)
		_ = getOpenVINOEP(cachePath, options)
	default:
		return nil, fmt.Errorf("unsupported execution provider: %x", ep)
	}

	return options, nil
}

func createMacOptions(cachePath string, ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, err
	}

	options.SetGraphOptimizationLevel(ort.GraphOptimizationLevelEnableAll)
	options.SetExecutionMode(ort.ExecutionModeParallel)

	switch ep {
	case types.ExecutionProviderCPU:
		return options, nil
	case types.ExecutionProviderCoreML:
		_ = getCoreMLEP(cachePath, options)
	case types.ExecutionProviderOpenVINO:
		_ = getOpenVINOEP(cachePath, options)
	case types.ExecutionProviderBest:
		_ = getCoreMLEP(cachePath, options)
		_ = getOpenVINOEP(cachePath, options)
	default:
		return nil, fmt.Errorf("unsupported execution provider: %x", ep)
	}

	return options, nil
}

// endregion
