package services

import (
	"gui/types"
	"runtime"
	"slices"

	"github.com/samber/lo"
	"github.com/vegidio/go-sak/async"
	"github.com/vegidio/go-sak/crypto"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct{}

func (d *DialogService) OpenFileDialog() ([]types.DialogFile, error) {
	dialog := application.OpenFileDialog()
	dialog.SetTitle("Select Image")
	dialog.AddFilter("Images (*.png;*.jpg)", "*.png;*.jpg")

	paths, err := dialog.PromptForMultipleSelection()
	if err != nil {
		return nil, err
	}

	ch := lo.SliceToChannel(1, paths)

	concurrentCh := async.ProcessChannel(ch, runtime.NumCPU(), func(path string) types.DialogFile {
		hash, _ := crypto.Xxh3File(path)
		return types.DialogFile{
			Path: path,
			Hash: hash,
		}
	})

	files := lo.ChannelToSlice(concurrentCh)

	// Keep the files sorted by path
	slices.SortFunc(files, func(a, b types.DialogFile) int {
		if a.Path < b.Path {
			return -1
		}
		if a.Path > b.Path {
			return 1
		}
		return 0
	})

	return files, nil
}
