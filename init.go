package openphotoai

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

const tag = "runtime/1.22.0"

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
//	err := opai.Initialize("myapp")
//	if err != nil {
//	    log.Fatal("Failed to initialize:", err)
//	}
//	defer opai.Destroy() // Clean up resources
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
//	if err := opai.Initialize("myapp"); err != nil {
//	    log.Fatal("Initialization failed:", err)
//	}
//	defer opai.Destroy() // Ensure cleanup on exit
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
	zipFile, err := fs.MkUserConfigFile(name, "onnxruntime-1.22.0.zip")
	if err != nil {
		return err
	}
	defer zipFile.Close()

	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/onnx_%s_%s.zip",
		tag, runtime.GOOS, runtime.GOARCH)

	err = utils.DownloadFile(url, zipFile)
	if err != nil {
		return err
	}
	defer os.Remove(zipFile.Name())

	targetDir := filepath.Dir(zipFile.Name())
	err = fs.Unzip(zipFile.Name(), targetDir)
	if err != nil {
		return err
	}

	return nil
}

func startRuntime(name string) error {
	configDir, err := fs.MkUserConfigDir(name)
	if err != nil {
		return err
	}

	// Add the application's config directory to the system PATH
	cudaPath := filepath.Join(configDir, "libs", "cuda")
	cudnnPath := filepath.Join(configDir, "libs", "cudnn")
	newPath := os.Getenv("PATH") + string(os.PathListSeparator) + cudaPath + string(os.PathListSeparator) + cudnnPath
	os.Setenv("PATH", newPath)

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
