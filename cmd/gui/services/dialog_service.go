package services

import (
	"gui/types"
	"image"
	_ "image/jpeg"
	_ "image/png"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"

	"github.com/samber/lo"
	"github.com/vegidio/go-sak/async"
	"github.com/vegidio/go-sak/crypto"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type DialogService struct{}

func (d *DialogService) OpenFileDialog() ([]types.File, error) {
	dialog := application.OpenFileDialog()
	dialog.SetTitle("Select Image")
	dialog.AddFilter("Images (*.png;*.jpg;*.tiff)", "*.png;*.jpg;*.jpeg;*.tif;*.tiff")

	paths, err := dialog.PromptForMultipleSelection()
	if err != nil {
		return nil, err
	}

	concurrentCh := async.SliceToChannel(paths, runtime.NumCPU(), func(path string) types.File {
		hash, _ := crypto.Xxh3File(path)
		dims, _ := getImageDimensions(path)

		// Extension
		ext := strings.ToLower(filepath.Ext(path))
		if len(ext) > 0 {
			ext = ext[1:]
		}

		// Size
		fileInfo, _ := os.Stat(path)
		size := int(fileInfo.Size())

		return types.File{
			Path:       path,
			Hash:       hash,
			Dimensions: dims,
			Extension:  ext,
			Size:       size,
		}
	})

	files := lo.ChannelToSlice(concurrentCh)

	// Keep the files sorted by path
	slices.SortFunc(files, func(a, b types.File) int {
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

func (d *DialogService) OpenDirDialog() (string, error) {
	dialog := application.OpenFileDialog()
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

// region - Private functions

func getImageDimensions(path string) ([]int, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	// DecodeConfig only reads the image header, not the full image
	config, _, err := image.DecodeConfig(file)
	if err != nil {
		return nil, err
	}

	return []int{config.Width, config.Height}, nil
}

// endregion
