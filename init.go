package opai

import (
	"fmt"
	"path/filepath"
	"runtime"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

const (
	onnxRuntimeZip = "onnxruntime-1.22.0.zip"
	onnxRuntimeTag = "runtime/1.22.0"
)

// Initialize sets up the model runtime by ensuring all required dependencies are available.
//
// This function performs a two-step initialization process:
//  1. Installs the ONNX runtime if not already present in the user's config directory
//  2. Initializes the ONNX runtime
//
// The name parameter specifies the application name used to create a dedicated config directory under the user's
// standard configuration path (e.g., ~/.config/name on Linux). It's important that you reuse the same name on later
// calls to Initialize() to ensure that the same config directory is used.
//
// Returns an error if any step fails, including:
//   - Unable to create config directories or files
//   - ONNX runtime initialization failures
//
// # Example:
//
//	err := opai.Initialize("myapp")
//	if err != nil {
//	    log.Fatal("Failed to initialize:", err)
//	}
//	defer opai.Destroy() // Clean up resources
func Initialize(name string) error {
	internal.AppName = name

	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/onnx_%s_%s.zip",
		onnxRuntimeTag, runtime.GOOS, runtime.GOARCH)

	if err := utils.PrepareDependency(url, "", onnxRuntimeZip, nil); err != nil {
		return err
	}

	// Initialize the ONNX runtime
	return startRuntime()
}

// Destroy cleans up resources used by the model runtime.
//
// This function performs cleanup operations to free memory and resources allocated during initialization. It should be
// called when the application is shutting down or when the AI functionalities are no longer needed.
//
// It's recommended to use this function with defer for proper cleanup:
//
// # Example:
//
//	if err := opai.Initialize("myapp"); err != nil {
//	    log.Fatal("Initialization failed:", err)
//	}
//	defer opai.Destroy() // Ensure cleanup on exit
func Destroy() {
	CleanRegistry()
	ort.DestroyEnvironment()
}

// region - Private functions

func startRuntime() error {
	configDir, err := fs.MkUserConfigDir(internal.AppName)
	if err != nil {
		return err
	}

	runtimePath := filepath.Join(configDir, onnxRuntimeName)
	ort.SetSharedLibraryPath(runtimePath)
	if err = ort.InitializeEnvironment(); err != nil {
		return err
	}

	// Disable ONNX runtime logging
	ort.SetEnvironmentLogLevel(ort.LoggingLevelFatal)

	return nil
}

// endregion
