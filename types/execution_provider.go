package types

// ExecutionProvider defines the execution provider used by the ONNX runtime.
type ExecutionProvider int

const (
	// ExecutionProviderCPU runs inference on the CPU.
	// This is the most compatible option but may be slower than hardware-accelerated providers.
	ExecutionProviderCPU ExecutionProvider = iota

	// ExecutionProviderTensorRT uses NVIDIA TensorRT for optimized inference on NVIDIA GPUs.
	// Requires TensorRT to be installed and available on the system.
	ExecutionProviderTensorRT

	// ExecutionProviderCUDA uses NVIDIA CUDA for GPU-accelerated inference on NVIDIA GPUs.
	// Requires CUDA to be installed and available on the system.
	ExecutionProviderCUDA

	// ExecutionProviderDirectML uses DirectML for hardware-accelerated inference on Windows.
	// Works with a wide range of DirectX 12 compatible GPUs on Windows 10 and later.
	ExecutionProviderDirectML

	// ExecutionProviderOpenVINO uses Intel OpenVINO for optimized inference on Intel hardware.
	// Supports Intel CPUs, integrated GPUs, and specialized AI accelerators.
	ExecutionProviderOpenVINO

	// ExecutionProviderCoreML uses Apple's Core ML framework for optimized inference on Apple devices.
	// Available on macOS and iOS devices with Apple Silicon or Intel processors.
	ExecutionProviderCoreML

	// ExecutionProviderBest automatically selects the best available execution provider
	// based on the current system's hardware and installed dependencies.
	ExecutionProviderBest
)
