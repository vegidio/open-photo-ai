package internal

import (
	"os"
	"runtime"
	"strconv"

	"github.com/vegidio/open-photo-ai/types"
)

// BudgetEnvVar overrides both pool budgets, in bytes, for support triage. Setting it to 0 makes the registry
// unbounded, which is the behaviour the app had before budgets existed.
const BudgetEnvVar = "OPAI_MODEL_BUDGET"

const (
	gibibyte = int64(1) << 30
	mebibyte = int64(1) << 20

	// bytesPerReportedMB converts what sysinfo.MemoryInfo actually returns into bytes.
	//
	// Its field is documented as bytes, but every platform backend divides by 1,000,000 before returning - decimal
	// megabytes, not bytes and not mebibytes. Taking the doc comment at its word makes a 64 GB machine look like 68 KB
	// of RAM, which silently collapses the host budget to its floor. Verified against the darwin (`sysctl
	// hw.memsize`), linux (`/proc/meminfo`) and windows (CIM) paths.
	bytesPerReportedMB = int64(1_000_000)

	// deviceBudgetFraction leaves headroom rather than filling the card. TensorRT alone is configured for a 4 GiB
	// workspace per session and cuDNN is allowed its maximum workspace, none of which shows up in the file sizes the
	// budget counts.
	deviceBudgetFraction = 70

	// unknownVramBudget is used when VRAM can't be queried, which is real: go-sak deliberately reports 0 on the
	// Windows CIM path because AdapterRAM is unreliable on modern cards, so AMD and Intel DirectML setups land here.
	// Falling back to the host rule would be worse than a guess - it would charge device memory against system RAM.
	unknownVramBudget = 4 * gibibyte

	minDeviceBudget = 1 * gibibyte

	// hostReserve is held back for the image pipeline, which competes for the same memory: a 24 MP photo upscaled 4x
	// is a 1.5 GB output buffer, and the disk cache then PNG-encodes that whole image into another buffer.
	hostReserve   = 4 * gibibyte
	minHostBudget = 1 * gibibyte
	maxHostBudget = 16 * gibibyte

	// fallbackHostRAM stands in for a machine whose RAM can't be queried. Small on purpose: guessing low costs a few
	// rebuilds, guessing high costs a swap storm.
	fallbackHostRAM = 8 * gibibyte
)

// PoolOf reports which memory a model built on ep occupies.
//
// Auto is the awkward one: the provider ONNX actually picks is decided inside the runtime and never reported back, so
// it is charged by platform. On macOS that is CoreML on unified memory, which is the host pool. Everywhere else Auto
// exists to reach a discrete GPU, so it is charged to the device pool when the machine has one - and to the host pool
// when it doesn't, since Auto then resolves to the CPU.
func PoolOf(ep types.ExecutionProvider) types.MemoryPool {
	switch ep {
	case types.ExecutionProviderCUDA, types.ExecutionProviderTensorRT, types.ExecutionProviderDirectML:
		return types.MemoryPoolDevice

	case types.ExecutionProviderAuto:
		if runtime.GOOS != "darwin" && hasDiscreteGPU() {
			return types.MemoryPoolDevice
		}

		return types.MemoryPoolHost

	default:
		// CPU, CoreML and OpenVINO. OpenVINO targets Intel integrated parts that share system RAM.
		return types.MemoryPoolHost
	}
}

// hasDiscreteGPU reports whether the machine has a GPU that reports its own VRAM. A card that reports none is either
// integrated or unqueryable, and in both cases the host pool is the safer place to charge it.
func hasDiscreteGPU() bool {
	gpus, err := GPUInfo()
	if err != nil {
		return false
	}

	for _, gpu := range gpus {
		if gpu.Memory > 0 {
			return true
		}
	}

	return false
}

// DefaultBudgets derives the per-pool ceilings for this machine, honouring BudgetEnvVar when it is set.
//
// Both are computed once, at Initialize, because the underlying sysinfo probes shell out to the OS.
func DefaultBudgets() (device, host int64) {
	if override, ok := budgetOverride(); ok {
		Log().Info("model memory budget overridden", "env", BudgetEnvVar, "bytes", override)
		return override, override
	}

	device, host = defaultDeviceBudget(), defaultHostBudget()
	Log().Info("model memory budgets", "device", device, "host", host)

	return device, host
}

// budgetOverride reads BudgetEnvVar. An unparseable or negative value is ignored with a warning rather than failing
// startup: this is a triage knob, and a typo in it should not stop the app from running.
func budgetOverride() (int64, bool) {
	raw, ok := os.LookupEnv(BudgetEnvVar)
	if !ok {
		return 0, false
	}

	bytes, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || bytes < 0 {
		Log().Warn("ignoring invalid model budget override", "env", BudgetEnvVar, "value", raw, "err", err)
		return 0, false
	}

	return bytes, true
}

// defaultDeviceBudget takes a fraction of the largest GPU's VRAM. The largest, not the sum: a model is built on one
// device, so the budget describes the card it will land on rather than the machine's total.
func defaultDeviceBudget() int64 {
	gpus, err := GPUInfo()
	if err != nil {
		Log().Warn("could not query GPU memory; using the default device budget", "err", err)
		return unknownVramBudget
	}

	var largestMiB uint
	for _, gpu := range gpus {
		largestMiB = max(largestMiB, gpu.Memory)
	}

	return deviceBudgetFor(largestMiB)
}

// defaultHostBudget takes half of system RAM less a fixed reserve for the image pipeline, clamped so that neither a
// tiny nor an enormous machine produces a nonsensical ceiling.
func defaultHostBudget() int64 {
	info, err := MemoryInfo()
	if err != nil {
		Log().Warn("could not query system memory; using the default host budget", "err", err)
		return hostBudgetFor(0)
	}

	return hostBudgetFor(info.Total)
}

// hostBudgetFor computes the host ceiling from the RAM figure sysinfo reports, in its own units. Split out from
// defaultHostBudget so the unit conversion and the clamps are testable without a machine of each size.
func hostBudgetFor(reportedMB uint64) int64 {
	total := int64(fallbackHostRAM)
	if reportedMB > 0 {
		total = int64(reportedMB) * bytesPerReportedMB
	}

	return min(max(total/2-hostReserve, minHostBudget), maxHostBudget)
}

// deviceBudgetFor computes the device ceiling from a VRAM figure in MiB, which is the unit sysinfo reports GPU memory
// in - a different unit from the one it reports system RAM in.
func deviceBudgetFor(vramMiB uint) int64 {
	if vramMiB == 0 {
		return unknownVramBudget
	}

	vram := int64(vramMiB) * mebibyte

	return max(vram*deviceBudgetFraction/100, minDeviceBudget)
}
