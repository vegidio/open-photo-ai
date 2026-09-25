package types

import (
	"slices"
	"strings"

	"github.com/cockroachdb/errors"
)

// ExecutionProvider defines the execution provider used by the ONNX runtime.
type ExecutionProvider string

const (
	// ExecutionProviderAuto automatically selects the best available execution provider
	// based on the current system's hardware and installed dependencies.
	ExecutionProviderAuto ExecutionProvider = "Auto"

	// ExecutionProviderCPU runs inference on the CPU.
	// This is the most compatible option but may be slower than hardware-accelerated providers.
	ExecutionProviderCPU ExecutionProvider = "CPU"

	// ExecutionProviderTensorRT uses NVIDIA TensorRT for optimized inference on NVIDIA GPUs.
	// Requires TensorRT to be installed and available on the system.
	ExecutionProviderTensorRT ExecutionProvider = "TensorRT"

	// ExecutionProviderCUDA uses NVIDIA CUDA for GPU-accelerated inference on NVIDIA GPUs.
	// Requires CUDA to be installed and available on the system.
	ExecutionProviderCUDA ExecutionProvider = "CUDA"

	// ExecutionProviderOpenVINO uses Intel OpenVINO for optimized inference on Intel hardware.
	// Supports Intel CPUs, integrated GPUs, and specialized AI accelerators.
	ExecutionProviderOpenVINO ExecutionProvider = "OpenVINO"

	// ExecutionProviderCoreML uses Apple's Core ML framework for optimized inference on Apple devices.
	// Available on macOS and iOS devices with Apple Silicon or Intel processors.
	ExecutionProviderCoreML ExecutionProvider = "CoreML"

	// ExecutionProviderWebGPU runs inference through the WebGPU API - Vulkan on Linux, Direct3D 12 on Windows and
	// Metal on macOS - so it reaches any GPU with a current driver, including the AMD and Intel ones the vendor
	// providers above leave on the CPU. It is a plugin provider: a small library the app downloads on first use, with
	// no vendor toolkit behind it.
	ExecutionProviderWebGPU ExecutionProvider = "WebGPU"
)

// AllExecutionProviders lists every published provider, in the order a user is most likely to reach for one. It is the
// single source for anything that has to enumerate them - a CLI flag's help text, a parser - so an eighth constant is
// reachable everywhere by declaring it here.
func AllExecutionProviders() []ExecutionProvider {
	return []ExecutionProvider{
		ExecutionProviderAuto,
		ExecutionProviderCPU,
		ExecutionProviderCoreML,
		ExecutionProviderCUDA,
		ExecutionProviderTensorRT,
		ExecutionProviderWebGPU,
		ExecutionProviderOpenVINO,
	}
}

// Valid reports whether ep is one of the published providers.
func (ep ExecutionProvider) Valid() bool {
	return slices.Contains(AllExecutionProviders(), ep)
}

// ParseExecutionProvider converts an untrusted string into an ExecutionProvider, rejecting anything that is not a
// published one. The match is case-insensitive: the constants' own spelling ("CoreML", "TensorRT") is not what anyone
// types on a command line or stores in a settings file.
//
// Same reasoning as ParsePrecision - anything building one of these from input it did not produce itself parses it
// here rather than converting it, so an unknown provider is refused at the boundary instead of reaching a session
// build as a silent no-op.
func ParseExecutionProvider(s string) (ExecutionProvider, error) {
	for _, ep := range AllExecutionProviders() {
		if strings.EqualFold(s, string(ep)) {
			return ep, nil
		}
	}

	return "", errors.Newf("unknown execution provider %q", s)
}
