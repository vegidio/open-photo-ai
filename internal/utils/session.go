package utils

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

func CreateSession(appName, modelName, tag string, onDownload func()) (*ort.DynamicAdvancedSession, error) {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return nil, err
	}

	// Download the model if it's not already present
	if url, yes := ShouldDownloadModel(appName, modelName, tag); yes {
		// Notify the user that the model will be downloaded
		if onDownload != nil {
			onDownload()
		}

		if err = DownloadModel(url, appName, modelName); err != nil {
			return nil, err
		}
	}

	var options *ort.SessionOptions

	// Check the computer's OS
	switch runtime.GOOS {
	case "windows":
		break
	case "linux":
		break
	case "darwin":
		options, err = createMacOptions(types.ExecutionProviderBest)
	}

	if err != nil {
		return nil, err
	}

	modelPath := filepath.Join(configDir, appName, "models", modelName)
	session, err := ort.NewDynamicAdvancedSession(modelPath, []string{"input"}, []string{"output"}, options)
	if err != nil {
		return nil, err
	}

	return session, nil
}

func createMacOptions(ep types.ExecutionProvider) (*ort.SessionOptions, error) {
	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, err
	}

	options.SetGraphOptimizationLevel(ort.GraphOptimizationLevelEnableAll)
	options.SetExecutionMode(ort.ExecutionModeParallel)
	options.SetIntraOpNumThreads(runtime.NumCPU())

	switch ep {
	case types.ExecutionProviderCPU:
		return options, nil
	case types.ExecutionProviderCoreML:
		_ = getCoreMLEP(options)
	case types.ExecutionProviderOpenVINO:
		_ = getOpenVINOEP(options)
	case types.ExecutionProviderBest:
		_ = getCoreMLEP(options)
		_ = getOpenVINOEP(options)
	default:
		return nil, fmt.Errorf("unsupported execution provider: %x", ep)
	}

	return options, nil
}

func getCoreMLEP(options *ort.SessionOptions) error {
	return options.AppendExecutionProviderCoreMLV2(map[string]string{
		"ModelFormat":              "MLProgram",
		"MLComputeUnits":           "ALL",
		"RequireStaticInputShapes": "0",
		"EnableOnSubgraphs":        "0",
	})
}

func getOpenVINOEP(options *ort.SessionOptions) error {
	return options.AppendExecutionProviderOpenVINO(map[string]string{
		"device_type":    "AUTO",
		"precision":      "FP32",
		"num_of_threads": fmt.Sprintf("%d", runtime.NumCPU()),
		"num_streams":    "2",
	})
}
