package services

import (
	"gui/types"
	"gui/utils"

	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct {
	app *application.App
}

func NewDialogService(app *application.App) *DialogService {
	return &DialogService{app: app}
}

func (s *DialogService) OpenFileDialog() ([]types.File, error) {
	dialog := s.app.Dialog.OpenFile()
	dialog.SetTitle("Select Image")
	dialog.AddFilter("Images (*.png;*.jpg;*.tiff)", "*.png;*.jpg;*.jpeg;*.tif;*.tiff")

	paths, err := dialog.PromptForMultipleSelection()
	if err != nil {
		return nil, err
	}

	files := utils.CreateFileTypes(paths)
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
