package utils

import (
	"gui/types"
	"image"
	_ "image/jpeg"
	_ "image/png"
	"log/slog"
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
		hash, err := crypto.Xxh3File(path)
		if err != nil {
			slog.Warn("failed to hash file", "path", path, "err", err)
		}

		dims, err := getImageDimensions(path)
		if err != nil {
			slog.Warn("failed to read image dimensions", "path", path, "err", err)
		}

		// Extension
		ext := strings.ToLower(filepath.Ext(path))
		if len(ext) > 0 {
			ext = ext[1:]
		}

		// Size — tolerate a missing/unreadable file; surrounding fields still carry useful info.
		var size int
		if fileInfo, err := os.Stat(path); err == nil {
			size = int(fileInfo.Size())
		} else {
			slog.Warn("failed to stat file", "path", path, "err", err)
		}

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
