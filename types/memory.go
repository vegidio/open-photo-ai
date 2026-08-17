package types

// MemoryPool identifies which kind of memory a loaded model occupies.
//
// The two are budgeted separately because they are not the same memory. Charging a 7 GB device-resident model against
// system RAM would admit it onto a graphics card that cannot hold it, and the failure surfaces as a silent drop to the
// CPU - slow inference with no error, which is harder to diagnose than a refusal.
type MemoryPool int

const (
	// MemoryPoolDevice is memory on a discrete accelerator: CUDA, TensorRT and DirectML keep model weights in VRAM.
	MemoryPoolDevice MemoryPool = iota

	// MemoryPoolHost is system RAM, where CPU inference keeps its weights. CoreML is charged here too: Apple Silicon
	// shares one physical pool between the CPU and the GPU, so there is no separate device memory to budget.
	MemoryPoolHost
)

// String implements fmt.Stringer so the pool reads as a name in logs and reports.
func (p MemoryPool) String() string {
	switch p {
	case MemoryPoolDevice:
		return "device"
	case MemoryPoolHost:
		return "host"
	default:
		return "unknown"
	}
}

// PoolMemory describes one pool's occupancy at a moment in time.
type PoolMemory struct {
	// Pool is which memory this describes.
	Pool MemoryPool

	// Resident is the total size of the model files currently loaded into this pool.
	//
	// It is a proxy, not a measurement: real footprint is larger, because arenas, cuDNN workspaces and the CoreML
	// MLProgram all sit on top of the weights and none of them is queryable through the ONNX bindings. The budgets are
	// set conservatively for exactly this reason.
	Resident int64

	// Budget is the ceiling Resident is kept under, or 0 when the pool is unbounded.
	Budget int64

	// Models is how many models are loaded in this pool.
	Models int
}

// ModelMemory reports what the model registry currently holds.
type ModelMemory struct {
	Device PoolMemory
	Host   PoolMemory
}
