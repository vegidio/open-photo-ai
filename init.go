package opai

import (
	"context"
	"path/filepath"
	"runtime"
	"sync"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

var destroyOnce sync.Once

// shutdownDrainTimeout bounds how long Destroy waits for in-flight inference to finish before giving up on a clean
// ONNX teardown. A single large upscale can legitimately run for a while, so it is generous.
const shutdownDrainTimeout = 30 * time.Second

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

	internal.Log().Info("initializing OPAI",
		"app_name", name, "onnx_tag", internal.ReleaseTag("onnx"), "os", runtime.GOOS, "arch", runtime.GOARCH)

	cache, err := internal.NewCache(500)
	if err != nil {
		return errors.Wrap(err, "failed to create image cache")
	}
	internal.ImageCache = cache

	// Drop what the execution providers compiled against an older runtime; the models themselves are plain ONNX graphs
	// and survive a runtime bump untouched.
	if err = cleanEngineCache(); err != nil {
		return errors.Wrap(err, "failed to clean the engine cache")
	}

	// ONNX Runtime
	runtimeDep, err := deps.ReleaseDependency("onnx-runtime", "onnx", internal.RuntimeDir)
	if err != nil {
		return errors.Wrap(err, "failed to describe the ONNX Runtime dependency")
	}

	if err = deps.Install(ctx, runtimeDep, onProgress); err != nil {
		return errors.Wrap(err, "failed to prepare ONNX Runtime")
	}

	// Sweep what older versions left behind, now that the runtime and the engine cache have directories of their own.
	// Neither failure is worth aborting a launch for: what is left over is wasted disk, not something that will be
	// loaded, since the runtime now in use is the one under RuntimeDir.
	pruneLegacyLayout()

	// Load model data. Without it there are no expected hashes, so every model downloaded this session is installed
	// unverified - worth saying plainly, since it used to be the silent outcome of a slow network.
	if modelData, err := utils.LoadModelData(); err == nil {
		internal.ModelData = modelData
	} else {
		internal.Log().Warn("no model manifest is available; models will be downloaded without verification this "+
			"session", "err", err)
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

func cleanEngineCache() error {
	tag := internal.ReleaseTag("onnx")

	wiped, err := utils.CleanEPCache(tag)
	if err != nil {
		return err
	}

	if wiped {
		internal.Log().Info("engine cache invalidated", "onnx_tag", tag)
	}

	return nil
}

// pruneLegacyLayout clears what installations predating the current directory layout left in place: the runtime that
// used to be extracted straight into the config root, and the provider caches that used to share the models directory.
//
// Neither prune's count is logged here: both already say what they removed, and the return value is there for the
// tests rather than for a second log line.
func pruneLegacyLayout() {
	if _, err := deps.PruneLegacyRuntime(); err != nil {
		internal.Log().Warn("failed to sweep the legacy runtime files", "err", err)
	}

	if _, err := deps.PruneLegacyEPCache(); err != nil {
		internal.Log().Warn("failed to sweep the legacy provider caches", "err", err)
	}
}

func startRuntime() error {
	configDir, err := fs.MkUserConfigDir(internal.AppName, internal.RuntimeDir)
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
