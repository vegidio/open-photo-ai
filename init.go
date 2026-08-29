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

// Initialize and Destroy are a lifecycle that may run more than once in a process, so "Destroy only does something
// once" has to mean once per Initialize rather than once ever. A plain sync.Once cannot be rearmed, hence the flag.
var (
	lifecycleMu sync.Mutex
	destroyed   bool
)

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

	// Rearm the lifecycle. A previous Destroy latched the registry closed and marked itself done; without clearing
	// both, this call would return successfully and then fail every single acquisition with ErrRegistryClosed.
	lifecycleMu.Lock()
	destroyed = false
	lifecycleMu.Unlock()
	internal.Registry.Reopen()

	onnxTag, _ := internal.ReleaseTag("onnx")

	internal.Log().Info("initializing OPAI",
		"app_name", name, "onnx_tag", onnxTag, "os", runtime.GOOS, "arch", runtime.GOARCH)

	cache, err := internal.NewCache(500)
	if err != nil {
		return errors.Wrap(err, "failed to create image cache")
	}
	internal.ImageCache = cache

	// Two slow, independent lookups nothing below needs until much later: the model manifest is an HTTPS request with a
	// five second timeout, and the memory budgets shell out to the OS. Started here, they run while the ONNX Runtime
	// downloads - which on a first launch is 175 MB - instead of adding their seconds after it. Both write only what
	// the joins below read.
	var (
		modelData []internal.RemoteModelData
		modelErr  error
		device    int64
		host      int64
		preludeWg sync.WaitGroup
	)

	preludeWg.Add(2)

	go func() {
		defer preludeWg.Done()
		modelData, modelErr = utils.LoadModelData()
	}()

	go func() {
		defer preludeWg.Done()
		device, host = internal.DefaultBudgets()
	}()

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

	// Join the prelude. Without the manifest there are no expected hashes, so every model downloaded this session is
	// installed unverified - worth saying plainly, since it used to be the silent outcome of a slow network.
	preludeWg.Wait()

	if modelErr == nil {
		internal.ModelData = modelData
	} else {
		internal.Log().Warn("no model manifest is available; models will be downloaded without verification this "+
			"session", "err", modelErr)
	}

	// Initialize the ONNX runtime
	if err = startRuntime(); err != nil {
		return err
	}

	// Bound how much stays resident. The defaults were derived from the machine by the prelude above; an embedder that
	// wants different ceilings calls SetModelBudget afterwards.
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
	lifecycleMu.Lock()
	defer lifecycleMu.Unlock()

	if destroyed {
		return
	}
	destroyed = true

	internal.Log().Info("destroying opai runtime")

	if internal.ImageCache != nil {
		// A failed flush loses cached results but changes nothing about the teardown, so it is logged rather than
		// returned - Destroy has no error to give and the process is on its way out.
		if err := internal.ImageCache.Close(); err != nil {
			internal.Log().Warn("failed to close the image cache", "err", err)
		}
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
}

// region - Private functions

func cleanEngineCache() error {
	tag, found := internal.ReleaseTag("onnx")
	if !found {
		return errors.New("the ONNX Runtime is not pinned to a release")
	}

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

	pinned, found := internal.PinnedArchive("onnx")
	if !found || pinned.Lib == "" {
		return errors.Newf("no ONNX Runtime is published for %s/%s", runtime.GOOS, runtime.GOARCH)
	}

	runtimePath := filepath.Join(configDir, pinned.Lib)
	ort.SetSharedLibraryPath(runtimePath)
	if err = ort.InitializeEnvironment(); err != nil {
		return errors.Wrap(err, "failed to initialize ONNX Runtime")
	}

	// ONNX Runtime logs from C++ to the process's stderr, which shared.SetupLogging redirects into opai.log - so this
	// level decides how much of it is worth keeping, not how much noise reaches a terminal. Warning is the useful
	// floor: it is where the node-assignment and provider-fallback diagnostics live, without the per-node flood that
	// Info and Verbose produce. It has to come after InitializeEnvironment, which is what the binding checks for.
	if err = ort.SetEnvironmentLogLevel(ort.LoggingLevelWarning); err != nil {
		internal.Log().Warn("failed to set the ONNX Runtime log level", "err", err)
	}

	internal.Log().Info("ONNX runtime started", "runtime_path", runtimePath)
	return nil
}

// endregion
