package utils

import (
	"path/filepath"
	"slices"
	"strings"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// webgpuEpName is what the plugin calls its provider in the devices it publishes; it is the key GetEpDevices is
// filtered on, and the one string in this file that has to match the library byte for byte.
const webgpuEpName = "WebGpuExecutionProvider"

// webgpuRegistration is the name the plugin library is registered under with the runtime environment. It is only a
// handle for Unregister, so it is ours to choose.
const webgpuRegistration = "webgpu"

// webgpuLib is the plugin library InitializeWebGPULib installed, empty until it has. The registration state beside
// it belongs to the ONNX environment that was up when the library was registered: the devices are owned by that
// environment and die with it, which is why ResetWebGPU exists and is called from Destroy.
var (
	webgpuMu         sync.Mutex
	webgpuLib        string
	webgpuRegistered bool
	webgpuDevices    []ort.EpDevice
)

// webgpuRunMu serializes Run across every session the WebGPU provider is attached to. Two sessions running at once on
// the plugin crash on Linux (microsoft/onnxruntime#32561); the library runs its operations one after another already,
// so in practice this only costs something when two enhancements overlap, which is exactly the case it protects.
var webgpuRunMu sync.Mutex

// errProviderUnavailable marks a provider that declined because it is not set up on this machine, as opposed to one
// that failed. The distinction decides how loudly the caller reports it: a provider the user never asked for, on a
// machine that never downloaded it, is not worth a warning on every session build.
var errProviderUnavailable = errors.New("execution provider is not available on this machine")

// SetWebGPULibrary records where the plugin library is, which is what makes the provider attachable. It is called
// by the public utils.InitializeWebGPULib once the download is verified; nothing here fetches anything.
func SetWebGPULibrary(lib string) {
	webgpuMu.Lock()
	defer webgpuMu.Unlock()

	webgpuLib = lib
}

// ResetWebGPU forgets the plugin registration. It has to be called when the ONNX environment is torn down, because
// the devices it holds were owned by that environment; the library path survives, since the files on disk do.
func ResetWebGPU() {
	webgpuMu.Lock()
	defer webgpuMu.Unlock()

	webgpuRegistered = false
	webgpuDevices = nil
}

// webgpuDevice registers the plugin with the runtime on first use and returns the device it publishes.
//
// Registration is deferred to the first session rather than done in InitializeWebGPULib because the library has to
// be registered with the environment that will build the sessions, and Initialize may bring that environment up more
// than once in a process - see ResetWebGPU.
func webgpuDevice() (ort.EpDevice, error) {
	webgpuMu.Lock()
	defer webgpuMu.Unlock()

	if webgpuLib == "" {
		return ort.EpDevice{}, errors.Wrap(errProviderUnavailable, "the WebGPU plugin is not installed")
	}

	// Tracked by its own flag rather than by the device slice being empty. A machine whose loader is present but
	// whose driver Dawn cannot use registers successfully and publishes nothing, and keying off the slice would
	// register the library again on the next model - which the runtime rejects as a duplicate, turning a quiet
	// "no GPU here" into a registration error on every session build.
	if !webgpuRegistered {
		if err := ort.RegisterExecutionProviderLibrary(webgpuRegistration, webgpuLib); err != nil {
			return ort.EpDevice{}, errors.Wrap(err, "failed to register the WebGPU plugin")
		}

		devices, err := ort.GetEpDevices()
		if err != nil {
			return ort.EpDevice{}, errors.Wrap(err, "failed to list the execution provider devices")
		}

		for _, device := range devices {
			if device.EpName() == webgpuEpName {
				webgpuDevices = append(webgpuDevices, device)
			}
		}

		webgpuRegistered = true
		internal.Log().Info("WebGPU plugin registered", "lib", webgpuLib, "devices", len(webgpuDevices))
	}

	if len(webgpuDevices) == 0 {
		// Registered fine, found nothing to run on: no Vulkan ICD, a driver too old for Dawn. Marked unavailable so
		// that Auto skips it quietly rather than warning on every model.
		return ort.EpDevice{}, errors.Wrap(errProviderUnavailable, "the WebGPU plugin found no usable GPU")
	}

	// The plugin accepts a single device and picks the physical GPU itself.
	return webgpuDevices[0], nil
}

// webgpuOptions is the provider configuration for one model. The defaults are the plugin's own; the profile's
// WebGPUOptions overlay them, which is how a model keeps a node the provider mishandles on the CPU.
func webgpuOptions(p EPProfile) map[string]string {
	options := map[string]string{}

	for key, value := range p.WebGPUOptions {
		options[key] = value
	}

	return options
}

// isFp16Graph reports whether a model file is one of the half-precision exports, by the naming convention every
// published model follows: `<category>_<codename>_fp16.onnx`.
func isFp16Graph(modelFile string) bool {
	stem := strings.TrimSuffix(modelFile, filepath.Ext(modelFile))
	return strings.HasSuffix(stem, "_"+string(types.PrecisionFp16))
}

// webgpuSupportsGraph reports whether the provider is trusted with a model. Today that is only the fp32 exports.
//
// The fp16 graphs come back wrong from this provider: measured against the fp32 CPU result, the transformer-style
// denoise and sharpen graphs disagree on 80-90% of pixels, some by more than a full stop, and even the plain
// convolutional upscalers drift by several 8-bit steps where the CPU's fp16 run stays within one. Until the plugin
// computes those in fp32 internally, as the CPU provider does, an fp16 request falls to the next provider in the
// chain rather than returning a slightly wrong image quickly.
func webgpuSupportsGraph(modelFile string) bool {
	return !isFp16Graph(modelFile)
}

// serializesRuns reports whether sessions built on this chain must not run concurrently with each other.
func serializesRuns(attached []types.ExecutionProvider) bool {
	return slices.Contains(attached, types.ExecutionProviderWebGPU)
}
