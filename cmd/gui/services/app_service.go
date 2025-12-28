package services

import (
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type AppService struct {
	app *application.App
	tel *o11y.Telemetry
}

const AppName = "open-photo-ai"

func NewAppService(app *application.App, tel *o11y.Telemetry) *AppService {
	return &AppService{
		app: app,
		tel: tel,
	}
}

func (s *AppService) Initialize() error {
	onProgress := func(_, _ int64, percent float64) {
		s.app.Event.Emit("app:download", "ONNX Runtime", percent)
	}

	// Initialize the model runtime
	if err := opai.Initialize(AppName, onProgress); err != nil {
		s.tel.LogError("Error initializing ONNX", nil, err)
		s.app.Event.Emit("app:download:error")
		return err
	}

	// Initialize CUDA and TensorRT if they are supported
	if utils.IsCudaSupported() {
		if err := s.initializeCuda(); err != nil {
			s.tel.LogError("Error initializing CUDA", nil, err)
			s.app.Event.Emit("app:download:error")
			return err
		}
	}

	//if utils.IsTensorRtSupported() {
	//	if err := s.initializeTensorRT(); err != nil {
	//		s.tel.LogError("Error initializing TensorRT", nil, err)
	//		s.app.Event.Emit("app:download:error")
	//		return err
	//	}
	//}

	return nil
}

func (s *AppService) Version() string {
	return utils.Version()
}

func (s *AppService) Destroy() {
	opai.Destroy()
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
