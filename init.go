package opai

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

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

	// shutdownDrainTimeout bounds how long Destroy waits for in-flight inference to finish before giving up on a clean
	// ONNX teardown. A single large upscale can legitimately run for a while, so it is generous.
	shutdownDrainTimeout = 30 * time.Second
)

// Initialize sets up the model runtime, downloading the ONNX runtime on first use and deriving the per-machine memory
// budgets. It must be called before any other function in this package.
//
// The name parameter specifies the application name used to create a dedicated config directory under the user's
// standard configuration path (e.g., ~/.config/name on Linux). It's important that you reuse the same name on later
// calls to Initialize() to ensure that the same config directory is used.
//
// Cancelling ctx aborts any in-flight download; already-downloaded files are kept for the next call.
//
// # Example:
//
//	err := opai.Initialize(ctx, "myapp", nil)
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

	// Invalidate the on-disk model cache when it predates the current ONNX runtime tag
	if err = cleanModelCache(); err != nil {
		return errors.Wrap(err, "failed to clean model cache")
	}

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

	// Bound how much stays resident. The defaults are derived from the machine here, once, because the probes behind
	// them shell out to the OS; an embedder that wants different ceilings calls SetModelBudget afterwards.
	device, host := internal.DefaultBudgets()
	internal.Registry.SetBudget(types.MemoryPoolDevice, device)
	internal.Registry.SetBudget(types.MemoryPoolHost, host)
	internal.Registry.SetIdleTTL(internal.DefaultIdleTTL)
	internal.Registry.StartJanitor()

	internal.Log().Info("opai initialized", "app_name", name)
	return nil
}

// Destroy unloads every model and tears the ONNX environment down. Call it at shutdown, typically with defer. Calling
// it more than once is harmless; only the first call does anything.
//
// # Example:
//
//	if err := opai.Initialize(ctx, "myapp", nil); err != nil {
//	    log.Fatal("Initialization failed:", err)
//	}
//	defer opai.Destroy() // Ensure cleanup on exit
func Destroy() {
	destroyOnce.Do(func() {
		internal.Log().Info("destroying opai runtime")

		if internal.ImageCache != nil {
			internal.ImageCache.Close()
		}

		// Tearing the ONNX environment down while a session is still running is the same use-after-free that freeing a
		// model would be, and at shutdown there is no one left to report it. So the environment comes down only once
		// every model is provably gone; if some work refuses to finish, leaking it is the right trade - the process is
		// exiting anyway, and the OS reclaims everything a moment later.
		if internal.Registry.Close(shutdownDrainTimeout) {
			ort.DestroyEnvironment()
			return
		}

		internal.Log().Error("timed out waiting for models to be released; skipping ONNX teardown to avoid a crash",
			"timeout", shutdownDrainTimeout)
	})
}

// region - Private functions

func cleanModelCache() error {
	modelsDir, err := fs.MkUserConfigDir(internal.AppName, "models")
	if err != nil {
		return errors.Wrap(err, "failed to resolve models directory")
	}

	versionPath := filepath.Join(modelsDir, ".version")

	current, readErr := os.ReadFile(versionPath)
	if readErr == nil && strings.TrimSpace(string(current)) == onnxRuntimeTag {
		return nil
	}

	entries, err := os.ReadDir(modelsDir)
	if err != nil {
		return errors.Wrap(err, "failed to read models directory")
	}
	for _, e := range entries {
		if err = os.RemoveAll(filepath.Join(modelsDir, e.Name())); err != nil {
			return errors.Wrapf(err, "failed to remove %s from model cache", e.Name())
		}
	}

	if err = os.WriteFile(versionPath, []byte(onnxRuntimeTag), 0o644); err != nil {
		return errors.Wrap(err, "failed to write model cache version file")
	}

	internal.Log().Info("model cache invalidated", "onnx_tag", onnxRuntimeTag)
	return nil
}

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
