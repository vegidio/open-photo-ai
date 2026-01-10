package services

import (
	"runtime"

	"github.com/wailsapp/wails/v3/pkg/application"
)

type OsService struct {
	app *application.App
}

func NewOsService(app *application.App) *OsService {
	return &OsService{app: app}
}

func (s *OsService) GetOS() string {
	return runtime.GOOS
}

func (s *OsService) GetArch() string {
	return runtime.GOARCH
}

func (s *OsService) RevealInFileManager(path string) error {
	return s.app.Env.OpenFileManager(path, true)
}

func (s *OsService) destroy() {
	// Nothing to do here
}
