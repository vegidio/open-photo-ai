package utils

import (
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
	CudaTag     = "cuda/12.9.1"
	CudnnTag    = "cudnn/9.14.0"
	TensorrtTag = "tensorrt/10.9.0"
)

// IsCudaSupported performs a simplified check whether the system has an NVIDIA GPU that possibly supports CUDA.
//
// Returns false if an error occurs while querying GPU information or no NVIDIA GPU is found.
func IsCudaSupported() bool {
	gpus, err := sysinfo.GetGPUInfo()
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

// IsTensorRtSupported performs a simplified check whether the system has an NVIDIA GPU that possibly supports TensorRT.
//
// Returns false if an error occurs while querying GPU information or no NVIDIA GPU is found.
func IsTensorRtSupported() bool {
	gpus, err := sysinfo.GetGPUInfo()
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

// InitializeNvidiaLib downloads and initializes an NVIDIA library dependency.
//
// It constructs a download URL based on the library name, tag, and current OS/architecture, then downloads and extracts
// the library to the user's config directory. The library path is added to the environment PATH for runtime discovery.
//
// # Parameters:
//   - libName: The name of the NVIDIA library (e.g., "cuda", "cudnn", "tensorrt").
//   - libTag: The version tag used in the download URL (e.g., "cuda/12.9.1").
//   - checkFile: A file path used to verify if the library is already installed.
//   - onProgress: A callback function to report download progress.
//
// Returns an error if the download, extraction, or path configuration fails.
func InitializeNvidiaLib(libName, libTag string, fileCheck *types.FileCheck, onProgress types.DownloadProgress) error {
	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/%s_%s_%s.zip",
		libTag, libName, runtime.GOOS, runtime.GOARCH)
	destination := filepath.Join("libs", libName)

	if err := utils.PrepareDependency(url, destination, fileCheck, onProgress); err != nil {
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
