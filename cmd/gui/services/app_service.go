package services

import (
	"context"
	"log/slog"
	"path/filepath"
	"sync"
	"sync/atomic"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/github"
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/shared"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type AppService struct {
	app  *application.App
	otel *o11y.Telemetry

	// initMu serializes Initialize, which the frontend can genuinely call twice: the mount effect and the download
	// dialog's "Try Again" are separate call sites, and Wails runs each binding call on its own goroutine. The one
	// that actually happens in production is the error boundary's Reload - window.location.reload() starts a fresh
	// frontend that calls Initialize again while this process is still inside the previous call. Without the lock
	// that means two ONNX Runtime downloads writing the same file, and two InitializeEnvironment calls racing the
	// IsInitialized check that is supposed to make a repeat call safe.
	initMu sync.Mutex

	// initialized and supportedEPs hold the result of the first *successful* call, so a retry arriving after the app
	// is already up answers from here instead of booting everything a second time. Only success is remembered: a
	// failed attempt has to stay retryable, which is the whole point of the button that triggers it.
	initialized  bool
	supportedEPs SupportedEPs

	// fallbackNotified keeps the "running on CPU" warning to one per set of loaded models, so a run that downgrades
	// several models only tells the user once. CleanRegistry clears it, so picking a different processor that also
	// fails is reported again.
	fallbackNotified atomic.Bool
}

type SupportedEPs struct {
	CUDA     bool
	TensorRT bool
	CoreML   bool
}

func NewAppService(app *application.App, otel *o11y.Telemetry) *AppService {
	return &AppService{
		app:  app,
		otel: otel,
	}
}

// Initialize boots the AI runtime and reports which execution providers this machine can actually use. It downloads
// the ONNX Runtime on first run, emitting EventAppDownload as it goes, so the frontend can show progress before any
// enhancement is possible. Callers must await it before invoking any inference service.
func (s *AppService) Initialize(ctx context.Context) (SupportedEPs, error) {
	s.initMu.Lock()
	defer s.initMu.Unlock()

	if s.initialized {
		slog.Info("app service is already initialized; returning the providers from the first call")
		return s.supportedEPs, nil
	}

	supportedEPs := SupportedEPs{}

	slog.Info("initializing app service")

	onProgress := func(_, _ int64, percent float64) {
		s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "ONNX Runtime", Percent: percent})
	}

	// Errors are surfaced through the returned error (the frontend awaits the promise and
	// maps rejections to UI state). We deliberately do not also emit an `app:download:error`
	// event here, to avoid two concurrent error paths racing in the UI.

	// Warn the user when a GPU processor turns out to be unusable and inference is downgraded to the CPU, so the
	// drop in speed doesn't look like the app hanging.
	opai.SetFallbackHandler(s.onProviderFallback)

	if err := opai.Initialize(ctx, shared.AppName, onProgress); err != nil {
		s.otel.LogError("Error initializing ONNX", nil, err)
		slog.Error("error initializing ONNX runtime", "err", err)
		return supportedEPs, errors.Wrap(err, "failed to initialize ONNX Runtime")
	}

	if utils.IsCudaSupported() {
		supportedEPs.CUDA = true
		slog.Info("CUDA supported")

		if err := s.initializeCuda(ctx); err != nil {
			s.otel.LogError("Error initializing CUDA", nil, err)
			slog.Error("error initializing CUDA", "err", err)
			return supportedEPs, errors.Wrap(err, "failed to initialize CUDA")
		}
	}

	if utils.IsTensorRtSupported() {
		supportedEPs.TensorRT = true
		slog.Info("TensorRT supported")

		if err := s.initializeTensorRT(ctx); err != nil {
			s.otel.LogError("Error initializing TensorRT", nil, err)
			slog.Error("error initializing TensorRT", "err", err)
			return supportedEPs, errors.Wrap(err, "failed to initialize TensorRT")
		}
	}

	// macOS only; the non-darwin build always reports false.
	if utils.IsCoreMLSupported() {
		supportedEPs.CoreML = true
		slog.Info("CoreML supported")
	}

	// The library falls back rather than failing when its cache directory is locked, and logs that to the log file
	// only - so unless it is reported here, a run with no cache is invisible from outside the machine. Deliberately
	// LogWarn and not LogError: this is a slower run, not a failed one, and it must not go back into the error budget
	// that the cache failing used to be a third of.
	if mode := opai.ImageCacheMode(); mode != types.CacheModeDisk {
		s.otel.LogWarn("Image cache degraded", map[string]any{"mode": string(mode)})
		slog.Warn("running with a degraded image cache", "mode", mode)
	}

	s.initialized = true
	s.supportedEPs = supportedEPs

	slog.Info("app service initialized",
		"cuda", supportedEPs.CUDA, "tensorrt", supportedEPs.TensorRT, "coreml", supportedEPs.CoreML)
	return supportedEPs, nil
}

// SetExecutionProvider tells the library the user picked a different AI processor.
//
// It unloads the models that were not built on the new choice. The registry is keyed by operation *and* provider, so
// the switch itself is an ordinary cache miss and needs no help; what needs help is the memory. Models built for the
// old processor stay resident for their full idle TTL - minutes - and on a GPU they hold VRAM the whole time the new
// ones are allocating beside them. Past the card's capacity Windows does not fail the allocation, it pages device
// memory to host RAM over PCIe, so the app keeps returning correct images an order of magnitude slower with nothing
// in the log to explain it.
//
// Nothing in use is waited on, so this is still safe in the middle of an export: the running job keeps the models it
// already holds and they are freed when it releases them; only the next job pays a rebuild.
//
// What it also resets is the two pieces of state that mean "this provider is bad": the library's latch, so the new
// choice actually gets tried instead of being short-circuited to the CPU, and the one-shot warning, so a downgrade on
// the new provider is news again.
func (s *AppService) SetExecutionProvider(ep types.ExecutionProvider) {
	s.fallbackNotified.Store(false)

	opai.UnloadModelsForProviderChange(ep)
}

// CleanRegistry unloads every model currently held in memory.
//
// Changing the AI processor no longer needs this - see SetExecutionProvider - so it exists for the case where the user
// wants the memory back now. It waits for any work still using a model before destroying it, so it is safe to call at
// any time; the frontend doesn't have to coordinate anything itself.
func (s *AppService) CleanRegistry() {
	// Unloading everything implies the same reset a provider change does - whatever was known to be bad is worth
	// trying again once nothing is loaded - but not the selective unload, which would keep whatever the current
	// processor happens to match. opai.CleanRegistry drains unconditionally and resets the latch itself.
	s.fallbackNotified.Store(false)

	opai.CleanRegistry()
}

// ModelMemory reports how much memory the loaded models are holding, for the diagnostics view.
func (s *AppService) ModelMemory() types.ModelMemory {
	return opai.ModelMemoryStats()
}

// Version returns the application version this build was stamped with, for the about dialog and bug reports.
func (s *AppService) Version() string {
	return shared.Version
}

// IsOutdated reports whether a newer release exists on GitHub. It makes a network call, so the frontend treats it as
// best-effort: a false result means "no newer release known", not "definitely up to date".
func (s *AppService) IsOutdated(ctx context.Context) bool {
	return github.IsOutdatedRelease(ctx, "vegidio", "open-photo-ai", shared.Version)
}

// GetLogsPath returns the absolute path to the application's log file. It mirrors how shared.SetupLogging builds the
// path so the two stay in sync.
func (s *AppService) GetLogsPath() (string, error) {
	logsDir, err := fs.MkUserConfigDir(shared.AppName, "logs")
	if err != nil {
		return "", errors.Wrap(err, "failed to resolve logs directory")
	}

	return filepath.Join(logsDir, "opai.log"), nil
}

// region - Private methods

func (s *AppService) destroy() {
	// Under the same lock that guards Initialize, and clearing the flag with it: the memoized result describes a
	// runtime that is about to stop existing, so leaving it set would let a later Initialize report providers for an
	// environment that has already been torn down.
	s.initMu.Lock()
	defer s.initMu.Unlock()

	s.initialized = false
	s.supportedEPs = SupportedEPs{}

	opai.Destroy()
}

// onProviderFallback reports that the requested execution provider couldn't create a model and the CPU was used
// instead. Only the first downgrade since the models were last loaded reaches the frontend; the rest are logged.
func (s *AppService) onProviderFallback(ep types.ExecutionProvider, err error) {
	// The library already reported the downgrade itself, with the operation and provider that caused it, so repeating
	// it here would put the same event in the log twice under two different wordings. What is only knowable at this
	// layer is whether the user was actually told - which is the question being asked when someone reports that the
	// app "went slow without saying anything".
	if s.fallbackNotified.Swap(true) {
		slog.Debug("execution provider fallback already reported to the user; not repeating it", "ep", ep)
		return
	}

	slog.Warn("notified the user of the CPU fallback", "ep", ep, "err", err)

	s.otel.LogError("Execution provider fallback", map[string]any{"provider": string(ep)}, err)
	s.app.Event.Emit(EventAppFallback, ProviderFallback{Provider: string(ep)})
}

func (s *AppService) initializeCuda(ctx context.Context) error {
	if err := utils.InitializeNvidiaLib(ctx, "cuda",
		func(_, _ int64, percent float64) {
			s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "NVIDIA CUDA", Percent: percent})
		}); err != nil {
		return errors.Wrap(err, "failed to download CUDA dependency")
	}

	if err := utils.InitializeNvidiaLib(ctx, "cudnn",
		func(_, _ int64, percent float64) {
			s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "NVIDIA cuDNN", Percent: percent})
		}); err != nil {
		return errors.Wrap(err, "failed to download cuDNN dependency")
	}

	return nil
}

func (s *AppService) initializeTensorRT(ctx context.Context) error {
	if err := utils.InitializeNvidiaLib(ctx, "tensorrt",
		func(_, _ int64, percent float64) {
			s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "NVIDIA TensorRT", Percent: percent})
		}); err != nil {
		return errors.Wrap(err, "failed to download TensorRT dependency")
	}

	return nil
}

// endregion
