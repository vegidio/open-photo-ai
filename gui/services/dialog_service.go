package services

import (
	"fmt"

	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct{}

func (d *DialogService) OpenFileDialog() {
	dialog := application.OpenFileDialog()
	dialog.SetTitle("Select Image")
	dialog.AddFilter("Images (*.png;*.jpg)", "*.png;*.jpg")

	if path, err := dialog.PromptForSingleSelection(); err == nil {
		fmt.Println(path)
	}
}
