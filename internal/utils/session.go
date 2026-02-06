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
