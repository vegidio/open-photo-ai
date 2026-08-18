package utils

import (
	"context"
	"path"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/os"
	"github.com/vegidio/go-sak/sysinfo"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/types"
)

// IsCudaSupported reports whether the machine has an NVIDIA GPU, which is only a rough proxy for CUDA being usable:
// the driver may still be missing or too old, which shows up as a session-build failure later.
func IsCudaSupported() bool {
	gpus, err := internal.GPUInfo()
	if err != nil {
		return false
	}

	_, found := lo.Find(gpus, func(gpu sysinfo.GPUInfo) bool {
		vendor := strings.ToLower(gpu.Vendor)
		product := strings.ToLower(gpu.Name)
		return vendor == "nvidia" || strings.Contains(product, "nvidia")
	})

	return found
}

// IsTensorRtSupported reports whether the machine has an RTX 20-series or newer card. Like IsCudaSupported, it is a
// rough proxy - it says nothing about whether the TensorRT libraries themselves will load.
func IsTensorRtSupported() bool {
	gpus, err := internal.GPUInfo()
	if err != nil {
		return false
	}

	_, found := lo.Find(gpus, func(gpu sysinfo.GPUInfo) bool {
		vendor := strings.ToLower(gpu.Vendor)
		product := strings.ToLower(gpu.Name)

		return vendor == "nvidia" &&
			(strings.Contains(product, "rtx 50") ||
				strings.Contains(product, "rtx 40") ||
				strings.Contains(product, "rtx 30") ||
				strings.Contains(product, "rtx 20"))
	})

	return found
}

// InitializeNvidiaLib downloads an NVIDIA library (libName being "cuda", "cudnn" or "tensorrt") into the user's config
// directory and appends it to PATH and LD_LIBRARY_PATH so the ONNX Runtime can dlopen it.
//
// The release it comes from is not named here: libName is also the prefix of the published archive, so the tag, the
// expected hash and the size all come from the pinned artifact table. Bumping a library version is a regeneration of
// that table, with nothing to keep in sync at this end.
//
// Verification is against the pinned hash of the archive, which is the whole reason these are worth installing through
// the manifest: a CUDA tree is several hundred files, and the previous check - "does a LICENSE.txt exist" - meant a
// half-downloaded library was trusted forever. The manifest also replaces the version stamp that used to guard the
// directory, and records what to delete when the tag moves rather than emptying the directory blindly.
//
// On Linux the LD_LIBRARY_PATH change only takes effect in a process started afterwards, since glibc reads the
// variable once at exec time - see setLibPathAndRestart in cmd/gui.
func InitializeNvidiaLib(ctx context.Context, libName string, onProgress types.DownloadProgress) error {
	dep, err := deps.ReleaseDependency(libName, libName, path.Join("libs", libName))
	if err != nil {
		return errors.Wrap(err, "failed to describe the NVIDIA library dependency")
	}

	if err = deps.Install(ctx, dep, onProgress); err != nil {
		return errors.Wrap(err, "failed to prepare NVIDIA library")
	}

	libPath, err := fs.MkUserConfigDir(internal.AppName, "libs", libName)
	if err != nil {
		return errors.Wrap(err, "failed to create NVIDIA library directory")
	}

	os.AppendEnvPath("PATH", libPath)
	os.AppendEnvPath("LD_LIBRARY_PATH", libPath)

	return nil
}
