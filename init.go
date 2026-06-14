package opai

import (
	"context"
	"fmt"
	"path/filepath"
	"runtime"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

var destroyOnce sync.Once

const (
	onnxRuntimeTag = "runtime/1.26.0"
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
// Cancelling ctx aborts any in-flight download; already-downloaded files are kept for the next call.
//
// Returns an error if any step fails, including:
//   - Unable to create config directories or files
//   - ONNX runtime initialization failures
//
// # Example:
//
//	err := opai.Initialize(ctx, "myapp",  nil)
//	if err != nil {
//	    log.Fatal("Failed to initialize:", err)
//	}
//	defer opai.Destroy() // Clean up resources
func Initialize(ctx context.Context, name string, onProgress types.DownloadProgress) error {
	internal.AppName = name

	internal.Log().Info("initializing opai",
		"app_name", name, "onnx_tag", onnxRuntimeTag, "os", runtime.GOOS, "arch", runtime.GOARCH)

	cache, err := internal.NewCache(500)
	if err != nil {
		return errors.Wrap(err, "failed to create image cache")
	}
	internal.ImageCache = cache

	fileCheck := &types.FileCheck{
		Path: internal.OnnxRuntimeName,
		Hash: internal.OnnxRuntimeHash,
	}

	// ONNX Runtime
	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/onnx_%s_%s.7z",
		onnxRuntimeTag, runtime.GOOS, runtime.GOARCH)

	if err = utils.PrepareDependency(ctx, url, "", fileCheck, onProgress); err != nil {
		return errors.Wrap(err, "failed to prepare ONNX Runtime")
	}

	// Load model data
	if modelData, err := utils.LoadModelData(); err == nil {
		internal.ModelData = modelData
	} else {
		internal.Log().Warn("failed to load remote model data; continuing without it", "err", err)
	}

	// Initialize the ONNX runtime
	if err = startRuntime(); err != nil {
		return err
	}

	internal.Log().Info("opai initialized", "app_name", name)
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
//	if err := opai.Initialize("myapp", nil); err != nil {
//	    log.Fatal("Initialization failed:", err)
//	}
//	defer opai.Destroy() // Ensure cleanup on exit
func Destroy() {
	destroyOnce.Do(func() {
		internal.Log().Info("destroying opai runtime")

		if internal.ImageCache != nil {
			internal.ImageCache.Close()
		}

		CleanRegistry()
		ort.DestroyEnvironment()
	})
}

// region - Private functions

func startRuntime() error {
	configDir, err := fs.MkUserConfigDir(internal.AppName)
	if err != nil {
		return errors.Wrap(err, "failed to create config directory")
	}

	runtimePath := filepath.Join(configDir, internal.OnnxRuntimeName)
	ort.SetSharedLibraryPath(runtimePath)
	if err = ort.InitializeEnvironment(); err != nil {
		return errors.Wrap(err, "failed to initialize ONNX Runtime")
	}

	// Disable ONNX runtime logging
	//ort.SetEnvironmentLogLevel(ort.LoggingLevelFatal)

	internal.Log().Info("ONNX runtime started", "runtime_path", runtimePath)
	return nil
}

// endregion
