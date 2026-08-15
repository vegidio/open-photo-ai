package services

import (
	"context"
	"log/slog"
	"path/filepath"
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

func (s *AppService) Initialize(ctx context.Context) (SupportedEPs, error) {
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

	// Initialize the model runtime
	if err := opai.Initialize(ctx, shared.AppName, onProgress); err != nil {
		s.otel.LogError("Error initializing ONNX", nil, err)
		slog.Error("error initializing ONNX runtime", "err", err)
		return supportedEPs, errors.Wrap(err, "failed to initialize ONNX Runtime")
	}

	// Initialize CUDA and TensorRT if they are supported
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

	// Check if CoreML is supported (macOS only)
	if utils.IsCoreMLSupported() {
		supportedEPs.CoreML = true
		slog.Info("CoreML supported")
	}

	slog.Info("app service initialized",
		"cuda", supportedEPs.CUDA, "tensorrt", supportedEPs.TensorRT, "coreml", supportedEPs.CoreML)
	return supportedEPs, nil
}

// CleanRegistry unloads every model currently held in memory, so the next enhancement rebuilds them - that's how a
// change to the AI processor takes effect, since the registry is keyed by operation ID only.
//
// The call blocks until the inference in flight has finished, so it's safe to make at any time; the frontend doesn't
// have to wait for anything itself.
func (s *AppService) CleanRegistry() {
	// The models are about to be rebuilt on the newly chosen processor, so a downgrade to the CPU is news again.
	s.fallbackNotified.Store(false)

	opai.CleanRegistry()
}

func (s *AppService) Version() string {
	return shared.Version
}

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
	opai.Destroy()
}

// onProviderFallback reports that the requested execution provider couldn't create a model and the CPU was used
// instead. Only the first downgrade since the models were last loaded reaches the frontend; the rest are logged.
func (s *AppService) onProviderFallback(ep types.ExecutionProvider, err error) {
	slog.Warn("execution provider unavailable; falling back to CPU", "ep", ep, "err", err)

	if s.fallbackNotified.Swap(true) {
		return
	}

	s.otel.LogError("Execution provider fallback", map[string]any{"provider": string(ep)}, err)
	s.app.Event.Emit(EventAppFallback, ProviderFallback{Provider: string(ep)})
}

func (s *AppService) initializeCuda(ctx context.Context) error {
	if err := utils.InitializeNvidiaLib(ctx, "cuda", utils.CudaTag, &types.FileCheck{Path: "LICENSE_CudaRT.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "NVIDIA CUDA", Percent: percent})
		}); err != nil {
		return errors.Wrap(err, "failed to download CUDA dependency")
	}

	if err := utils.InitializeNvidiaLib(ctx, "cudnn", utils.CudnnTag, &types.FileCheck{Path: "LICENSE.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "NVIDIA cuDNN", Percent: percent})
		}); err != nil {
		return errors.Wrap(err, "failed to download cuDNN dependency")
	}

	return nil
}

func (s *AppService) initializeTensorRT(ctx context.Context) error {
	if err := utils.InitializeNvidiaLib(ctx, "tensorrt", utils.TensorrtTag, &types.FileCheck{Path: "LICENSE.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit(EventAppDownload, DownloadProgress{Dependency: "NVIDIA TensorRT", Percent: percent})
		}); err != nil {
		return errors.Wrap(err, "failed to download TensorRT dependency")
	}

	return nil
}

// endregion
