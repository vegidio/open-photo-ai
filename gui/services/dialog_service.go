package services

import (
	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct{}

func (d *DialogService) OpenFileDialog() ([]string, error) {
	dialog := application.OpenFileDialog()
	dialog.SetTitle("Select Image")
	dialog.AddFilter("Images (*.png;*.jpg)", "*.png;*.jpg")

	paths, err := dialog.PromptForMultipleSelection()
	if err != nil {
		return nil, err
	}

	return paths, nil
}
