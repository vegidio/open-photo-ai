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
	label := strings.Join(extensions, ";")

	dialog := s.app.Dialog.OpenFile()
	dialog.SetTitle(title)
	// Only the word is translated; the extension list is derived from what the decoder supports, so it stays here.
	dialog.AddFilter(filterName+" ("+label+")", filterPatterns(utils.SupportedInputExtensions()))

	paths, err := dialog.PromptForMultipleSelection()
	if err != nil {
		// Dismissing the picker is a normal choice, not a fault, so it is reported as info and returns an empty
		// selection - the frontend already treats that as "the user added nothing".
		if isDialogCancelled(err) {
			s.otel.LogInfo("File dialog cancelled", nil)
			slog.Info("file dialog cancelled")
			return []types.File{}, nil
		}

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
		// See OpenFileDialog: cancelling returns the empty path, which leaves any destination already chosen alone
		// instead of resetting it the way the error path does.
		if isDialogCancelled(err) {
			s.otel.LogInfo("Directory dialog cancelled", nil)
			slog.Info("directory dialog cancelled")
			return "", nil
		}

		s.otel.LogError("Error opening directory dialog", nil, err)
		slog.Warn("error opening directory dialog", "err", err)
		return "", errors.Wrap(err, "failed to open directory dialog")
	}

	slog.Info("directory selected", "path", path)
	return path, nil
}

// isDialogCancelled reports whether err is the Windows file picker's way of saying the user dismissed the dialog.
//
// Wails returns cfd.ErrorCancelled here, but that sentinel lives under wails/v3/internal, so errors.Is has nothing to
// compare against and the message is all there is to match on. macOS and Linux report a cancel as an empty selection
// with no error, so this only ever fires on Windows.
func isDialogCancelled(err error) bool {
	if err == nil {
		return false
	}

	msg := strings.ToLower(err.Error())
	return strings.Contains(msg, "cancelled by user") || strings.Contains(msg, "canceled by user")
}

func (s *DialogService) destroy() {}

// filterPatterns builds the dialog's glob list with each extension in both lower and upper case. GTK matches glob
// patterns case-sensitively on Linux, so "*.dng" alone hides a camera's "L1041576.DNG" - and cameras that name files
// in upper case are the rule, not the exception. macOS and Windows match case-insensitively; the duplicates are harmless.
func filterPatterns(extensions []string) string {
	patterns := make([]string, 0, len(extensions)*2)
	for _, ext := range extensions {
		patterns = append(patterns, "*."+ext)
		if upper := strings.ToUpper(ext); upper != ext {
			patterns = append(patterns, "*."+upper)
		}
	}
	return strings.Join(patterns, ";")
}
