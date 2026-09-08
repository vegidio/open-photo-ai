package internal

import (
	"context"
	"os"
	"runtime"
	"strconv"
	"sync"

	"github.com/vegidio/open-photo-ai/types"
)

// BudgetEnvVar overrides both pool budgets, in bytes, for support triage. Setting it to 0 makes the registry
// unbounded, which is the behaviour the app had before budgets existed.
const BudgetEnvVar = "OPAI_MODEL_BUDGET"

// OverheadEnvVar overrides deviceOverheadPercent, for the same triage reasons and because that constant is an
// estimate. Setting it to 0 charges device models their file bytes alone, which is what the app did before.
const OverheadEnvVar = "OPAI_MODEL_OVERHEAD_PERCENT"

const (
	gibibyte = int64(1) << 30

	// deviceBudgetFraction leaves headroom rather than filling the card. TensorRT alone is configured for a 4 GiB
	// workspace per session, and cuDNN is allowed its maximum workspace, none of which shows up in the file sizes the
	// budget counts.
	deviceBudgetFraction = 70

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

	// deviceOverheadPercent is what a resident GPU session costs *on top of* its weights, as a percentage of them.
	//
	// The budget counts model-file bytes, and for a device session that is an undercount rather than an
	// approximation: the weights are joined on the card by ONNX Runtime's CUDA arena for activations, the cuDNN
	// workspace that cudaOptions deliberately maximises (cudnn_conv_use_max_workspace=1 with an EXHAUSTIVE algo
	// search), and a TensorRT execution context's own device memory. None of that appears in a file size.
	//
	// Undercounting it is not a tidy accounting flaw, because of how the GPU fails. On Windows, WDDM does not refuse
	// an allocation that no longer fits in VRAM - the driver silently pages device memory to host RAM over PCIe, so
	// the app keeps returning correct images with no error anywhere and runs 10-60x slower. A budget that admits more
	// than the card holds produces exactly that, and produces it invisibly.
	//
	// 50% is a deliberately round, conservative estimate and NOT a measurement - nobody has profiled the real
	// per-session device footprint of these graphs. It is set where it is because the failure modes are asymmetric:
	// charging too much costs an occasional rebuild, and charging too little costs a silent 10-60x. Anyone tightening
	// it should measure resident VRAM (nvidia-smi, or NVML) across the model set at a realistic input size rather
	// than reasoning about it, and OPAI_MODEL_OVERHEAD_PERCENT overrides it in the meantime.
	deviceOverheadPercent = 50

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
	case types.ExecutionProviderCUDA, types.ExecutionProviderTensorRT:
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
//
// ctx only decides whether the probes are *started*, not whether a running one is abandoned: they go through go-sak's
// memoized GetGPUInfo/GetMemoryInfo, which own the subprocess and expose no way to cancel it. That still matters -
// an Initialize cancelled before this point no longer pays seconds for numbers nobody will read - but a probe already
// in flight runs to completion. Cancelling yields the same ceilings an unqueryable machine gets.
func DefaultBudgets(ctx context.Context) (device, host int64) {
	if override, ok := envInt64(BudgetEnvVar); ok {
		Log().Info("model memory budget overridden", "env", BudgetEnvVar, "bytes", override)
		return override, override
	}

	if err := ctx.Err(); err != nil {
		Log().Warn("skipping the memory probes because initialization was cancelled; using the default budgets",
			"err", err)
		return deviceBudgetFor(0), hostBudgetFor(0)
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

// envInt64 reads a non-negative int64 from the named environment variable, reporting false when it is unset.
//
// An unparseable or negative value is ignored with a warning rather than failing startup: every caller is a triage
// knob, and a typo in one should not stop the app from running.
func envInt64(name string) (int64, bool) {
	raw, ok := os.LookupEnv(name)
	if !ok {
		return 0, false
	}

	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || value < 0 {
		Log().Warn("ignoring invalid environment override", "env", name, "value", raw, "err", err)
		return 0, false
	}

	return value, true
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

// chargedBytes is what an entry costs its pool: its model-file bytes, plus - on the device pool - an allowance for the
// per-session memory those files do not describe. See deviceOverheadPercent for what that allowance stands for and why
// it errs high.
//
// The pool is what carries the distinction, and it is enough on its own. Both callers derive it with PoolOf from the
// provider the model was actually built on, so the CPU fallback needs no separate guard: a model that failed on CUDA
// and rebuilt on the CPU is charged against PoolOf(CPU), which is the host pool, and never reaches the allowance.
//
// A zero size means "unknown", not "free" - EstimateModelBytes returns 0 for anything the manifest can't name - and
// stays 0 here so that an unknown model is not charged an allowance on a size nobody has.
func chargedBytes(pool types.MemoryPool, bytes int64) int64 {
	if bytes <= 0 || pool != types.MemoryPoolDevice {
		return bytes
	}

	return bytes + bytes*overheadPercent()/100
}

// overheadPercent reads OverheadEnvVar, falling back to deviceOverheadPercent.
func overheadPercent() int64 {
	if percent, ok := envInt64(OverheadEnvVar); ok {
		return percent
	}

	return deviceOverheadPercent
}
