package services

import (
	"runtime"

	"github.com/wailsapp/wails/v3/pkg/application"
)

// OsService exposes the handful of host-platform facts and actions the frontend needs. Every exported method here is
// callable from JS through the generated bindings.
type OsService struct {
	app *application.App
}

func NewOsService(app *application.App) *OsService {
	return &OsService{app: app}
}

// GetOS returns the operating system this build is running on, as a Go GOOS value ("darwin", "windows", "linux").
// The frontend uses it to pick platform-specific wording and shortcuts.
func (s *OsService) GetOS() string {
	return runtime.GOOS
}

// GetArch returns the CPU architecture this build is running on, as a Go GOARCH value ("arm64", "amd64").
func (s *OsService) GetArch() string {
	return runtime.GOARCH
}

// RevealInFileManager opens the platform file manager with path selected — Finder on macOS, Explorer on Windows —
// so the user can get to an exported image or the log file without typing the path.
func (s *OsService) RevealInFileManager(path string) error {
	return s.app.Env.OpenFileManager(path, true)
}

func (s *OsService) destroy() {}
