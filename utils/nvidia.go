package utils

import (
	"context"
	"fmt"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/os"
	"github.com/vegidio/go-sak/sysinfo"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

const (
	CudaTag     = "cuda/13.3.0"
	CudnnTag    = "cudnn/9.23.1"
	TensorrtTag = "tensorrt/10.14.1"
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

// InitializeNvidiaLib downloads an NVIDIA library (libName being "cuda", "cudnn" or "tensorrt", libTag its release
// tag) into the user's config directory and appends it to PATH and LD_LIBRARY_PATH so the ONNX Runtime can dlopen it.
//
// On Linux the LD_LIBRARY_PATH change only takes effect in a process started afterwards, since glibc reads the
// variable once at exec time - see setLibPathAndRestart in cmd/gui.
func InitializeNvidiaLib(
	ctx context.Context,
	libName, libTag string,
	fileCheck *types.FileCheck,
	onProgress types.DownloadProgress,
) error {
	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/%s_%s_%s.7z",
		libTag, libName, runtime.GOOS, runtime.GOARCH)
	destination := filepath.Join("libs", libName)

	if err := utils.PrepareDependency(ctx, url, destination, fileCheck, onProgress); err != nil {
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
