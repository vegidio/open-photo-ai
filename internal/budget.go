package internal

import (
	"os"
	"runtime"
	"strconv"
	"sync"

	"github.com/vegidio/open-photo-ai/types"
)

// BudgetEnvVar overrides both pool budgets, in bytes, for support triage. Setting it to 0 makes the registry
// unbounded, which is the behaviour the app had before budgets existed.
const BudgetEnvVar = "OPAI_MODEL_BUDGET"

const (
	gibibyte = int64(1) << 30

	// deviceBudgetFraction leaves headroom rather than filling the card. TensorRT alone is configured for a 4 GiB
	// workspace per session, and cuDNN is allowed its maximum workspace, none of which shows up in the file sizes the
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

	// defaultResidentBytes is what a model that doesn't implement types.Measurable is charged.
	//
	// It is deliberately large - bigger than every model shipped today except the fp32 denoisers - because the failure
	// modes are asymmetric. Over-charging an unmeasurable model makes the budget conservative and costs at most a
	// rebuild; under-charging it (or treating it as free) lets an unbounded amount of memory accumulate outside the
	// accounting, which is the exact problem the budget exists to solve.
	defaultResidentBytes = 256 << 20
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
	_, ok := LargestVRAMBytes()
	return ok
}

// DefaultBudgets derives the per-pool ceilings for this machine, honouring BudgetEnvVar when it is set.
//
// Both are computed once, at Initialize, because the underlying sysinfo probes shell out to the OS - `system_profiler`
// on macOS, two separate PowerShell CIM queries on Windows. They are independent, so they run concurrently rather than
// adding both latencies to startup.
func DefaultBudgets() (device, host int64) {
	if override, ok := budgetOverride(); ok {
		Log().Info("model memory budget overridden", "env", BudgetEnvVar, "bytes", override)
		return override, override
	}

	var wg sync.WaitGroup

	wg.Go(func() {
		device = defaultDeviceBudget()
	})

	host = defaultHostBudget()
	wg.Wait()

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

// defaultDeviceBudget takes a fraction of the largest GPU's VRAM.
func defaultDeviceBudget() int64 {
	vram, ok := LargestVRAMBytes()
	if !ok {
		Log().Warn("could not determine GPU memory; using the default device budget")
	}

	return deviceBudgetFor(vram)
}

// defaultHostBudget takes half of system RAM less a fixed reserve for the image pipeline, clamped so that neither a
// tiny nor an enormous machine produces a nonsensical ceiling.
func defaultHostBudget() int64 {
	total := TotalRAMBytes()
	if total == 0 {
		Log().Warn("could not query system memory; using the default host budget")
	}

	return hostBudgetFor(total)
}

// hostBudgetFor computes the host ceiling from total system RAM in bytes, falling back when that is 0. Split out from
// defaultHostBudget so the clamps are testable without a machine of each size.
func hostBudgetFor(totalBytes int64) int64 {
	if totalBytes <= 0 {
		totalBytes = fallbackHostRAM
	}

	return min(max(totalBytes/2-hostReserve, minHostBudget), maxHostBudget)
}

// deviceBudgetFor computes the device ceiling from a VRAM figure in bytes, falling back when that is 0.
func deviceBudgetFor(vramBytes int64) int64 {
	if vramBytes <= 0 {
		return unknownVramBudget
	}

	return max(vramBytes*deviceBudgetFraction/100, minDeviceBudget)
}
