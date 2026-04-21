package services

import (
	"context"
	"shared"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/github"
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type AppService struct {
	app  *application.App
	otel *o11y.Telemetry
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

	onProgress := func(_, _ int64, percent float64) {
		s.app.Event.Emit("app:download", "ONNX Runtime", percent)
	}

	// Errors are surfaced through the returned error (the frontend awaits the promise and
	// maps rejections to UI state). We deliberately do not also emit an `app:download:error`
	// event here, to avoid two concurrent error paths racing in the UI.

	// Initialize the model runtime
	if err := opai.Initialize(ctx, shared.AppName, onProgress); err != nil {
		s.otel.LogError("Error initializing ONNX", nil, err)
		return supportedEPs, errors.Wrap(err, "failed to initialize ONNX Runtime")
	}

	// Initialize CUDA and TensorRT if they are supported
	if utils.IsCudaSupported() {
		supportedEPs.CUDA = true

		if err := s.initializeCuda(ctx); err != nil {
			s.otel.LogError("Error initializing CUDA", nil, err)
			return supportedEPs, errors.Wrap(err, "failed to initialize CUDA")
		}
	}

	if utils.IsTensorRtSupported() {
		supportedEPs.TensorRT = true

		if err := s.initializeTensorRT(ctx); err != nil {
			s.otel.LogError("Error initializing TensorRT", nil, err)
			return supportedEPs, errors.Wrap(err, "failed to initialize TensorRT")
		}
	}

	// Check if CoreML is supported (macOS only)
	if utils.IsCoreMLSupported() {
		supportedEPs.CoreML = true
	}

	return supportedEPs, nil
}

func (s *AppService) CleanRegistry() {
	opai.CleanRegistry()
}

func (s *AppService) Version() string {
	return shared.Version
}

func (s *AppService) IsOutdated() bool {
	return github.IsOutdatedRelease("vegidio", "open-photo-ai", shared.Version)
}

// region - Private methods

func (s *AppService) destroy() {
	opai.Destroy()
}

func (s *AppService) initializeCuda(ctx context.Context) error {
	if err := utils.InitializeNvidiaLib(ctx, "cuda", utils.CudaTag, &types.FileCheck{Path: "LICENSE_CudaRT.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA CUDA", percent)
		}); err != nil {
		return errors.Wrap(err, "failed to download CUDA dependency")
	}

	if err := utils.InitializeNvidiaLib(ctx, "cudnn", utils.CudnnTag, &types.FileCheck{Path: "LICENSE.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA cuDNN", percent)
		}); err != nil {
		return errors.Wrap(err, "failed to download cuDNN dependency")
	}

	return nil
}

func (s *AppService) initializeTensorRT(ctx context.Context) error {
	if err := utils.InitializeNvidiaLib(ctx, "tensorrt", utils.TensorrtTag, &types.FileCheck{Path: "LICENSE.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA TensorRT", percent)
		}); err != nil {
		return errors.Wrap(err, "failed to download TensorRT dependency")
	}

	return nil
}

// endregion
