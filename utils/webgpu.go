package utils

import (
	"context"
	"os"
	"path"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// IsWebGPUSupported reports whether the machine can plausibly drive the WebGPU provider: the plugin is published for
// this platform, a GPU is present, and the graphics API the plugin uses here is there to reach it.
//
// Like IsCudaSupported it is a proxy for the rest. The plugin enumerates adapters itself when it is registered, and a
// machine that passes here may still publish no device - a Vulkan loader with no ICD behind it, say. That case is
// caught at session build and falls to the next provider in the chain, having cost a download of a few megabytes
// rather than the gigabyte the NVIDIA check guards against. So the hardware half of this errs towards yes.
//
// The published-archive half does not, and it is the half that is not a heuristic: Microsoft builds the plugin for
// four platforms and the app runs on six, so on Linux and Windows ARM there is nothing to install. Asking here rather
// than letting InitializeWebGPULib fail is what keeps that from being an error the caller has to interpret - the GUI
// would log a warning on every launch, and cmd/perf would refuse to benchmark at all.
func IsWebGPUSupported() bool {
	if _, published := internal.PinnedArchive("webgpu"); !published {
		return false
	}

	gpus, err := internal.GPUInfo()
	if err != nil || len(gpus) == 0 {
		return false
	}

	switch runtime.GOOS {
	case "linux":
		return hasVulkanLoader()

	case "windows", "darwin":
		// Direct3D 12 and Metal ship with the operating system.
		return true

	default:
		return false
	}
}

// hasVulkanLoader reports whether libvulkan.so.1 is where the dynamic linker would find it. The plugin dlopens the
// loader at registration, so a machine without it publishes no device and the download would be wasted.
//
// The standard directories are spelled out rather than parsed from ld.so.cache because the list is short and stable,
// and a false negative here only means the CPU - the same outcome as before this provider existed.
func hasVulkanLoader() bool {
	dirs := []string{
		"/usr/lib", "/usr/lib64", "/usr/local/lib", "/lib", "/lib64",
		"/usr/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu",
	}

	if extra := os.Getenv("LD_LIBRARY_PATH"); extra != "" {
		dirs = append(strings.Split(extra, string(os.PathListSeparator)), dirs...)
	}

	for _, dir := range dirs {
		if _, err := os.Stat(filepath.Join(dir, "libvulkan.so.1")); err == nil {
			return true
		}
	}

	return false
}

// InitializeWebGPULib downloads the WebGPU plugin library into the user's config directory and records where it is,
// so the provider can be attached to sessions built from now on.
//
// Unlike the NVIDIA libraries this needs no LD_LIBRARY_PATH and no restart: the plugin is loaded by path through the
// runtime's own registration API, and the only thing it dlopens itself is the platform's graphics loader.
func InitializeWebGPULib(ctx context.Context, onProgress types.DownloadProgress) error {
	dep, err := deps.ReleaseDependency("webgpu", "webgpu", path.Join("libs", "webgpu"))
	if err != nil {
		return errors.Wrap(err, "failed to describe the WebGPU plugin dependency")
	}

	if err = deps.Install(ctx, dep, onProgress); err != nil {
		return errors.Wrap(err, "failed to prepare the WebGPU plugin")
	}

	pinned, _ := internal.PinnedArchive("webgpu")

	libDir, err := internal.ConfigDir(dep.Destination)
	if err != nil {
		return err
	}

	lib := filepath.Join(libDir, pinned.Lib)
	utils.SetWebGPULibrary(lib)
	internal.Log().Info("WebGPU plugin ready", "lib", lib)

	return nil
}
