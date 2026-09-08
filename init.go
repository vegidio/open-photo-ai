package opai

import (
	"context"
	"path/filepath"
	"runtime"
	"sync"
	"time"

	"github.com/cockroachdb/errors"
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

// imageCacheMu serializes setupImageCache against itself, because its check-then-swap is not atomic on its own. Two
// overlapping Initialize calls would both find no cache installed and both open one; each open is a handle that owes
// the store a Close, only one of them would end up installed, and the other would be dropped unreferenced - leaving
// the store open for the life of the process no matter how carefully Destroy closes what it can reach.
//
// Overlapping calls are reachable: the GUI's error boundary reloads the frontend, which starts a second Initialize
// while the Go side is still inside the first.
var imageCacheMu sync.Mutex

// imageCacheEntries bounds how many processed images the disk cache keeps. The store is bounded by bytes as well, and
// that is the limit that actually binds for full-resolution results; this one is what stops a long session of small
// previews from accumulating entries indefinitely underneath that ceiling.
const imageCacheEntries = 500

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
	internal.SetAppName(name)

	// Rearm the lifecycle. A previous Destroy latched the registry closed and marked itself done; without clearing
	// both, this call would return successfully and then fail every single acquisition with ErrRegistryClosed.
	lifecycleMu.Lock()
	destroyed = false
	lifecycleMu.Unlock()
	internal.Registry.Reopen()

	onnxTag, _ := internal.ReleaseTag("onnx")

	internal.Log().Info("initializing OPAI",
		"app_name", name, "onnx_tag", onnxTag, "os", runtime.GOOS, "arch", runtime.GOARCH)

	// Two slow, independent lookups nothing below needs until much later: the model manifest is an HTTPS request with a
	// five-second timeout, and the memory budgets shell out to the OS. Started here, they run while the ONNX Runtime
	// downloads - that on a first launch are 175 MB - instead of adding their seconds after it. Both write only what
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
		modelData, modelErr = utils.LoadModelData(ctx)
	}()

	go func() {
		defer preludeWg.Done()
		device, host = internal.DefaultBudgets(ctx)
	}()

	// Several failures below return before the join at the bottom, and a goroutine still writing to modelData or device
	// after Initialize has returned is a data race the moment a caller retries. The explicit Wait further down is what
	// the happy path uses; this one only ever does anything on an error return.
	defer preludeWg.Wait()

	// Drop what the execution providers compiled against an older runtime; the models themselves are plain ONNX graphs
	// and survive a runtime bump untouched.
	if err := cleanEngineCache(); err != nil {
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
		internal.SetModelData(modelData)
	} else {
		internal.Log().Warn("no model manifest is available; models will be downloaded without verification this "+
			"session", "err", modelErr)
	}

	// Initialize the ONNX runtime
	if err = startRuntime(); err != nil {
		return err
	}

	// Last, and deliberately: the cache is the one thing here the app can run without, so it is set up only once
	// everything the app cannot run without has succeeded. It used to be first, and a cache that would not open
	// aborted the launch before the runtime had even been downloaded.
	setupImageCache()

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

	// Clearing the pointer is what makes the close safe, not just tidy: Process guards on a non-nil cache, so leaving
	// a closed store reachable would turn a stray call after Destroy into a use-after-close on the badger handle.
	if cache := internal.SwapImageCache(nil); cache != nil {
		// A failed flush loses cached results but changes nothing about the teardown, so it is logged rather than
		// returned - Destroy has no error to give and the process is on its way out.
		if err := cache.Close(); err != nil {
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

// setupImageCache installs the image cache for this lifecycle, degrading rather than failing.
//
// The order here used to be inverted, and that was the bug. Opening the new store first and swapping afterwards meant
// a second Initialize with no Destroy between them asked Badger for the directory lock this very process was already
// holding, so the open failed, Initialize returned at that error, and the swap that would have closed the previous
// store was never reached. The recovery was unreachable in exactly the case it was written for. The GUI reaches that
// case through its error boundary: "Reload" is window.location.reload(), which remounts the frontend and calls
// Initialize again inside a Go process whose Badger handle survived.
//
// memo.NewDiskShared now makes the deadlock itself impossible - a path this process already holds comes back as
// another handle onto the same store - but the reuse below still earns its place twice over. Every open is a handle
// that owes the store a Close, and Initialize runs more often than Destroy does, so opening on each call would leak
// references until nothing could ever close the store. And it protects in-flight work: Process binds the cache
// pointer for a whole call, so swapping a store out from under a running enhancement is a use-after-close.
//
// The one case that does reopen is a cache directory that moved, which means the caller passed a different app name
// to Initialize. Two different directories, so nothing is contended either way, and there the previous store is
// closed before the new one is opened rather than after.
//
// Nothing here returns an error, and nothing here is fatal. Losing the cache costs speed; ImageCache() being nil is a
// state Process already supports and runs uncached under.
func setupImageCache() {
	imageCacheMu.Lock()
	defer imageCacheMu.Unlock()

	// cmd/cli and cmd/perf turn the cache off before calling Initialize, precisely because a cached result would
	// invalidate what they measure, and they were still paying to open the store - and still taking the directory
	// lock that a GUI running beside them then failed on.
	//
	// Note that this does not close a store already open: an embedder that disables the cache and re-initializes
	// keeps the lock until Destroy. Closing it here would mean closing under an in-flight Process that has already
	// captured the pointer, which is the hazard the rest of this function exists to avoid.
	if !internal.ImageCacheEnabled() {
		internal.Log().Info("the image cache is disabled; no store will be opened")
		return
	}

	cachePath, err := internal.ConfigDir("cache")
	if err != nil {
		internal.Log().Warn("could not resolve the image cache directory; this run will not cache results", "err", err)
		return
	}

	if current := internal.ImageCache(); current != nil && current.Path() == cachePath {
		internal.Log().Debug("reusing the image cache already open for this process",
			"path", cachePath, "mode", current.Mode())
		return
	}

	// Whatever is installed is no longer the store this app name resolves to, so it has to go - and it has to go
	// before the new open, since leaving it would leak a Badger handle nothing can reach any more.
	if previous := internal.SwapImageCache(nil); previous != nil {
		if err = previous.Close(); err != nil {
			internal.Log().Warn("failed to close the previous image cache", "err", err)
		}
	}

	cache, err := internal.NewCache(imageCacheEntries)
	if err == nil {
		internal.SwapImageCache(cache)
		internal.Log().Info("image cache ready", "mode", cache.Mode(), "path", cachePath)
		return
	}

	// Another copy of the app holds the lock, or the directory is unwritable. Neither has a retry that can succeed,
	// so this run caches in RAM instead of failing. Self-inflicted lock contention no longer reaches here - that is
	// what NewDiskShared removed - so an error at this point really does mean something outside this process.
	internal.Log().Warn("the disk image cache could not be opened; caching in memory for this run",
		"path", cachePath, "err", err)

	if cache, err = internal.NewMemoryCache(imageCacheEntries); err != nil {
		// Nothing left to fall back to, and nothing that needs to: the pointer stays nil and every operation is
		// recomputed rather than read back.
		internal.Log().Error("no image cache is available for this run; every operation will be recomputed", "err", err)
		return
	}

	internal.SwapImageCache(cache)
	internal.Log().Info("image cache ready", "mode", cache.Mode())
}

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
	configDir, err := internal.ConfigDir(internal.RuntimeDir)
	if err != nil {
		return err
	}

	pinned, found := internal.PinnedArchive("onnx")
	if !found || pinned.Lib == "" {
		return errors.Newf("no ONNX Runtime is published for %s/%s", runtime.GOOS, runtime.GOARCH)
	}

	runtimePath := filepath.Join(configDir, pinned.Lib)

	// Initialize may run more than once in a process, and a Destroy that timed out deliberately skips the teardown to
	// avoid crashing on a live session - so the environment can still be up when we get here. The binding returns an
	// error rather than a no-op in that case, which would fail every Initialize after the first, so the environment is
	// only built when there isn't one. The library path is set inside the same guard because it is only read by
	// InitializeEnvironment; re-pointing it at an already-loaded runtime would do nothing.
	if !ort.IsInitialized() {
		ort.SetSharedLibraryPath(runtimePath)
		if err = ort.InitializeEnvironment(); err != nil {
			return errors.Wrap(err, "failed to initialize ONNX Runtime")
		}
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
