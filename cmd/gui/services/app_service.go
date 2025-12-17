package services

import (
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type AppService struct {
	app *application.App
}

const AppName = "open-photo-ai"

func NewAppService(app *application.App) *AppService {
	return &AppService{app: app}
}

func (s *AppService) Initialize() error {
	onProgress := func(_, _ int64, percent float64) {
		s.app.Event.Emit("app:download", "ONNX Runtime", percent)
	}

	// Initialize the model runtime
	if err := opai.Initialize(AppName, onProgress); err != nil {
		s.app.Event.Emit("app:download:error")
		return err
	}

	// Initialize CUDA and TensorRT if they are supported
	if utils.IsCudaSupported() {
		if err := s.initializeCuda(); err != nil {
			s.app.Event.Emit("app:download:error")
			return err
		}
	}

	if utils.IsTensorRtSupported() {
		if err := s.initializeTensorRT(); err != nil {
			s.app.Event.Emit("app:download:error")
			return err
		}
	}

	return nil
}

func (s *AppService) Destroy() {
	defer opai.Destroy()
}

// region - Private methods

func (s *AppService) initializeCuda() error {
	if err := utils.InitializeNvidiaLib("cuda", utils.CudaTag, "LICENSE_CudaRT.txt",
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA CUDA", percent)
		}); err != nil {
		return err
	}

	if err := utils.InitializeNvidiaLib("cudnn", utils.CudnnTag, "LICENSE.txt",
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA cuDNN", percent)
		}); err != nil {
		return err
	}

	return nil
}

func (s *AppService) initializeTensorRT() error {
	if err := utils.InitializeNvidiaLib("tensorrt", utils.TensorrtTag, "LICENSE.txt",
		func(_, _ int64, percent float64) {
			s.app.Event.Emit("app:download", "NVIDIA TensorRT", percent)
		}); err != nil {
		return err
	}

	return nil
}

// endregion
