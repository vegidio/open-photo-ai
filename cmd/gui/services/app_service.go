package services

import (
	opai "github.com/vegidio/open-photo-ai"
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
		return err
	}

	return nil
}

func (s *AppService) Destroy() {
	defer opai.Destroy()
}
