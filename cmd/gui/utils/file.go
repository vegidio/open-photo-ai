package utils

import (
	"gui/types"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"

	"github.com/samber/lo"
	"github.com/vegidio/go-sak/async"
	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/open-photo-ai/utils"
)

// IsSupportedFile reports whether path has a supported image extension.
func IsSupportedFile(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))
	if len(ext) > 0 {
		ext = ext[1:]
	}
	return slices.Contains(utils.SupportedInputExtensions(), ext)
}

// PartitionSupportedFiles splits paths into supported and unsupported by extension.
func PartitionSupportedFiles(paths []string) (supported, unsupported []string) {
	for _, path := range paths {
		if IsSupportedFile(path) {
			supported = append(supported, path)
		} else {
			unsupported = append(unsupported, path)
		}
	}
	return supported, unsupported
}

func CreateFileTypes(paths []string) []types.File {
	concurrentCh := async.SliceToChannel(paths, runtime.NumCPU(), func(path string) types.File {
		hash, err := crypto.Xxh3File(path)
		if err != nil {
			slog.Warn("failed to hash file", "path", path, "err", err)
		}

		dims, err := utils.ImageDimensions(path)
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
