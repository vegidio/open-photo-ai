package utils

import (
	"context"
	"path"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/os"
	"github.com/vegidio/go-sak/sysinfo"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/types"
)

// IsCudaSupported reports whether the machine has an NVIDIA GPU that the pinned CUDA toolkit can actually target.
//
// It remains a proxy for CUDA being usable - the driver may still be missing or too old, which shows up as a
// session-build failure later - but it is no longer only a vendor check. The architecture the toolkit was built for is
// knowable up front, and getting it wrong is expensive: a true answer here is what makes AppService.Initialize
// download the CUDA and cuDNN trees, roughly a gigabyte, before the first session fails and everything falls back to
// the CPU anyway.
func IsCudaSupported() bool {
	return hasGpuMeetingFloor("cuda")
}

// IsTensorRtSupported reports whether the machine has an NVIDIA GPU that the pinned TensorRT release can target.
//
// Like IsCudaSupported it says nothing about whether the libraries themselves will load, and it is the more expensive
// of the two to get wrong: the TensorRT archives are around 1.4-2 GB, several times the CUDA tree.
//
// This used to match model names - "rtx 20" through "rtx 50" - which was wrong in both directions. It rejected every
// Turing card NVIDIA did not brand RTX, so the GTX 1650 and 1660 were refused a release that supports them, and it
// accepted "RTX 2000 Ada Generation" by way of the "rtx 20" prefix while refusing its Quadro and RTX A-series
// siblings. The compute capability answers the question the names were standing in for, and needs no new entry each
// time a product line is renamed.
//
// TensorRT's floor can never sit below CUDA's, because the TensorRT execution provider is built on the CUDA one - see
// TestTensorRtFloorIsNotBelowCuda, which pins that rather than leaving it to be rediscovered.
func IsTensorRtSupported() bool {
	return hasGpuMeetingFloor("tensorrt")
}

// hasGpuMeetingFloor reports whether any NVIDIA GPU in the machine clears the compute capability floor pinned for dep.
//
// A dependency that declares no floor is unconstrained, which keeps this honest for anything published without one:
// the question then collapses back to "is there an NVIDIA card".
func hasGpuMeetingFloor(dep string) bool {
	gpus, err := internal.GPUInfo()
	if err != nil {
		return false
	}

	floor, hasFloor := internal.MinComputeCapability(dep)

	for _, gpu := range gpus {
		if !internal.IsNvidia(gpu) {
			continue
		}

		if !hasFloor || canTarget(gpu.ComputeCapability, floor) {
			return true
		}

		// Said out loud, because the alternative is an NVIDIA machine where a processor option simply is not there.
		// The tag is in the line so the answer to "why not?" does not require knowing which version this build pins.
		tag, _ := internal.ReleaseTag(dep)
		internal.Log().Info("GPU is too old for the pinned release; not offering this processor",
			"dependency", dep, "gpu", gpu.Name, "compute_capability", gpu.ComputeCapability.String(),
			"minimum", floor.String(), "version", tag)
	}

	return false
}

// canTarget reports whether a card is new enough for a release with the given compute capability floor.
//
// A card whose capability could not be read is treated as usable. That is deliberate, and it is the only judgement
// call in this file. Reading it the other way would take a processor away from a machine on the strength of a probe
// that failed - nvidia-smi missing from PATH, or a driver predating the compute_cap field - and the symptom would be a
// silently slower app on hardware that was fine. Guessing wrong in this direction costs a wasted download and a
// fallback the user is already told about, which is exactly what happens today; guessing wrong in the other direction
// is a regression nobody can see. The cards this is meant to catch all report their capability perfectly well.
func canTarget(capability, floor sysinfo.ComputeCapability) bool {
	if !capability.Known() {
		return true
	}

	return capability.AtLeast(floor.Major, floor.Minor)
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

	// dep.Destination rather than a second copy of the same join: Install resolved that exact path, so the directory
	// added to the loader's search path is by construction the one the files were written to.
	libPath, err := internal.ConfigDir(dep.Destination)
	if err != nil {
		return err
	}

	os.AppendEnvPath("PATH", libPath)
	os.AppendEnvPath("LD_LIBRARY_PATH", libPath)

	return nil
}
