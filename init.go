package openphotoai

import (
	"log"
	"os"
	"path/filepath"

	"github.com/vegidio/go-sak/fs"
	ort "github.com/yalue/onnxruntime_go"
)

var appName = "open-photo-ai"

// Initialize sets up the model runtime by ensuring all required dependencies are available.
//
// This function performs a two-step initialization process:
//  1. Installs the ONNX runtime library if not already present in the user's config directory
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
//	err := openphotoai.Initialize("myapp")
//	if err != nil {
//	    log.Fatal("Failed to initialize:", err)
//	}
//	defer openphotoai.Destroy() // Clean up resources
func Initialize(name string) error {
	appName = name

	// Install the ONNX runtime if it's not already installed
	if yes := shouldInstallRuntime(appName); yes {
		if err := installRuntime(appName); err != nil {
			return err
		}
	}

	// Initialize the ONNX runtime
	if err := startRuntime(appName); err != nil {
		return err
	}

	return nil
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
//	if err := openphotoai.Initialize("myapp"); err != nil {
//	    log.Fatal("Initialization failed:", err)
//	}
//	defer openphotoai.Destroy() // Ensure cleanup on exit
func Destroy() {
	CleanRegistry()
	ort.DestroyEnvironment()
}

// region - Private functions

func shouldInstallRuntime(name string) bool {
	configDir, err := fs.MkUserConfigDir(name)
	if err != nil {
		log.Fatalf("error getting user config directory: %v\n", err)
	}

	runtimePath := filepath.Join(configDir, onnxRuntimeName)
	_, err = os.Stat(runtimePath)
	return os.IsNotExist(err)
}

func installRuntime(name string) error {
	file, err := fs.MkUserConfigFile(name, onnxRuntimeName)
	if err != nil {
		return err
	}
	defer file.Close()

	_, err = file.Write(onnxRuntimeBinary)
	if err != nil {
		return err
	}

	return nil
}

func startRuntime(name string) error {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return err
	}

	runtimePath := filepath.Join(configDir, name, onnxRuntimeName)

	ort.SetSharedLibraryPath(runtimePath)
	if err = ort.InitializeEnvironment(); err != nil {
		return err
	}

	// Disable ONNX runtime logging
	ort.SetEnvironmentLogLevel(ort.LoggingLevelFatal)

	return nil
}

// endregion
