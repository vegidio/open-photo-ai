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

// OpenFileDialog prompts the user to pick one or more images, filtered to the formats the decoder supports, and
// returns them as loaded File records. The slice is empty when the user cancels, which is not an error.
//
// title and filterName arrive already translated from the frontend, which owns the i18n catalog. Passing them in
// keeps a second, Go-side catalog - and the job of keeping it in sync - out of the backend entirely.
func (s *DialogService) OpenFileDialog(title string, filterName string) ([]types.File, error) {
	extensions := lo.Map(utils.SupportedInputExtensions(), func(ext string, _ int) string {
		return "*." + ext
	})
	extFilter := strings.Join(extensions, ";")

	dialog := s.app.Dialog.OpenFile()
	dialog.SetTitle(title)
	// Only the word is translated; the extension list is derived from what the decoder supports, so it stays here.
	dialog.AddFilter(filterName+" ("+extFilter+")", extFilter)

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

// OpenDirDialog prompts the user to pick a single directory, used as the destination for a batch export. The path is
// empty when the user cancels, which is not an error. title arrives already translated - see OpenFileDialog.
func (s *DialogService) OpenDirDialog(title string) (string, error) {
	dialog := s.app.Dialog.OpenFile()
	dialog.SetTitle(title)
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
