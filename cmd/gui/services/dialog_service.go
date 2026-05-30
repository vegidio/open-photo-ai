package services

import (
	"gui/types"
	guiutils "gui/utils"
	"log/slog"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/o11y"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct {
	app  *application.App
	otel *o11y.Telemetry
}

func NewDialogService(app *application.App, otel *o11y.Telemetry) *DialogService {
	return &DialogService{app: app, otel: otel}
}

func (s *DialogService) OpenFileDialog() ([]types.File, error) {
	extensions := lo.Map(utils.SupportedImageExtensions(), func(ext string, _ int) string {
		return "*." + ext
	})
	extFilter := strings.Join(extensions, ";")

	dialog := s.app.Dialog.OpenFile()
	dialog.SetTitle("Select Image")
	dialog.AddFilter("Images ("+extFilter+")", extFilter)

	paths, err := dialog.PromptForMultipleSelection()
	if err != nil {
		s.otel.LogError("Error opening file dialog", nil, err)
		slog.Warn("error opening file dialog", "err", err)
		return nil, errors.Wrap(err, "failed to open file dialog")
	}

	files := guiutils.CreateFileTypes(paths)
	slog.Info("files selected", "count", len(files))
	return files, nil
}

func (s *DialogService) OpenDirDialog() (string, error) {
	dialog := s.app.Dialog.OpenFile()
	dialog.SetTitle("Select Directory")
	dialog.CanChooseFiles(false)
	dialog.CanChooseDirectories(true)
	dialog.CanCreateDirectories(true)

	path, err := dialog.PromptForSingleSelection()
	if err != nil {
		s.otel.LogError("Error opening directory dialog", nil, err)
		slog.Warn("error opening directory dialog", "err", err)
		return "", errors.Wrap(err, "failed to open directory dialog")
	}

	slog.Info("directory selected", "path", path)
	return path, nil
}

func (s *DialogService) destroy() {}
