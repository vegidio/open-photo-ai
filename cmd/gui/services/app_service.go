package services

import (
	"shared"

	"github.com/vegidio/go-sak/github"
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type AppService struct {
	app *application.App
	tel *o11y.Telemetry
}

type SupportedEPs struct {
	CUDA     bool
	TensorRT bool
	CoreML   bool
}

func NewAppService(app *application.App, tel *o11y.Telemetry) *AppService {
	return &AppService{
		app: app,
		tel: tel,
	}
}

func (s *AppService) Initialize() (SupportedEPs, error) {
	supportedEPs := SupportedEPs{}

	onProgress := func(_, _ int64, percent float64) {
		s.app.Event.Emit("app:download", "ONNX Runtime", percent)
	}

	// Initialize the model runtime
	if err := opai.Initialize(shared.AppName, onProgress); err != nil {
		s.tel.LogError("Error initializing ONNX", nil, err)
		s.app.Event.Emit("app:download:error")
		return supportedEPs, err
	}

	// Initialize CUDA and TensorRT if they are supported
	if utils.IsCudaSupported() {
		supportedEPs.CUDA = true

		if err := s.initializeCuda(); err != nil {
			s.tel.LogError("Error initializing CUDA", nil, err)
			s.app.Event.Emit("app:download:error")
			return supportedEPs, err
		}
	}

	if utils.IsTensorRtSupported() {
		supportedEPs.TensorRT = true

		if err := s.initializeTensorRT(); err != nil {
			s.tel.LogError("Error initializing TensorRT", nil, err)
			s.app.Event.Emit("app:download:error")
			return supportedEPs, err
		}
	}

	// Check if CoreML is supported (macOS only)
	if utils.IsCoreMLSupported() {
		supportedEPs.CoreML = true
	}

	return supportedEPs, nil
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

func (s *AppService) initializeCuda() error {
	if err := utils.InitializeNvidiaLib("cuda", utils.CudaTag, &types.FileCheck{Path: "LICENSE_CudaRT.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA CUDA", percent)
		}); err != nil {
		return err
	}

	if err := utils.InitializeNvidiaLib("cudnn", utils.CudnnTag, &types.FileCheck{Path: "LICENSE.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA cuDNN", percent)
		}); err != nil {
		return err
	}

	return nil
}

func (s *AppService) initializeTensorRT() error {
	if err := utils.InitializeNvidiaLib("tensorrt", utils.TensorrtTag, &types.FileCheck{Path: "LICENSE.txt"},
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA TensorRT", percent)
		}); err != nil {
		return err
	}

	return nil
}

// endregion
