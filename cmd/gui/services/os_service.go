package services

import (
	"github.com/wailsapp/wails/v3/pkg/application"
)

type OsService struct {
	app *application.App
}

func NewOsService(app *application.App) *OsService {
	return &OsService{app: app}
}

func (s *OsService) RevealInFileManager(path string) error {
	return s.app.Env.OpenFileManager(path, true)
}
