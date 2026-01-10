package services

import (
	"gui/types"
	guiutils "gui/utils"
	"strings"

	"github.com/samber/lo"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct {
	app *application.App
}

func NewDialogService(app *application.App) *DialogService {
	return &DialogService{app: app}
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
		return nil, err
	}

	files := guiutils.CreateFileTypes(paths)
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
		return "", err
	}

	return path, nil
}

func (s *DialogService) destroy() {
	// Nothing to do here
}
