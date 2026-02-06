package utils

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

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/async"
	"github.com/vegidio/go-sak/crypto"
)

func CreateFileTypes(paths []string) []types.File {
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

	return files
}

func getImageDimensions(path string) ([]int, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, errors.Wrap(err, "failed to open image file")
	}
	defer file.Close()

	// DecodeConfig only reads the image header, not the full image
	config, _, err := image.DecodeConfig(file)
	if err != nil {
		return nil, errors.Wrap(err, "failed to decode image config")
	}

	return []int{config.Width, config.Height}, nil
}
