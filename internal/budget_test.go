package internal

import (
	"runtime"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// TestHostBudgetFor pins the clamps. The unit conversion that feeds it lives in sysinfo.go and is covered by
// TestTotalRAMBytes.
func TestHostBudgetFor(t *testing.T) {
	const gib = int64(1) << 30

	tests := []struct {
		name       string
		totalBytes int64
		want       int64
	}{
		// Half the machine, less the 4 GiB pipeline reserve.
		{"16 GB machine", 16 * gib, 4 * gib},
		{"32 GB machine", 32 * gib, 12 * gib},
		{"64 GB machine is capped", 64 * gib, 16 * gib},
		{"an enormous machine is capped", 1024 * gib, 16 * gib},

		// Half of 8 GiB less a 4 GiB reserve is 0, so the floor applies.
		{"8 GB machine hits the floor", 8 * gib, 1 * gib},
		{"a tiny machine hits the floor", 2 * gib, 1 * gib},

		// Unqueryable RAM falls back to the assumed machine size rather than to zero.
		{"unknown RAM uses the fallback", 0, 1 * gib},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := hostBudgetFor(tt.totalBytes); got != tt.want {
				t.Errorf("hostBudgetFor(%.3f GiB) = %.3f GiB, want %.3f GiB",
					float64(tt.totalBytes)/float64(gib), float64(got)/float64(gib), float64(tt.want)/float64(gib))
			}
		})
	}
}

// TestDeviceBudgetFor pins the device-side fraction and its clamps.
func TestDeviceBudgetFor(t *testing.T) {
	const gib = int64(1) << 30
	const mib = int64(1) << 20

	tests := []struct {
		name      string
		vramBytes int64
		want      int64
	}{
		{"8 GB card", 8192 * mib, 8192 * mib * 70 / 100},
		{"12 GB card", 12288 * mib, 12288 * mib * 70 / 100},
		{"unqueryable VRAM uses the fallback", 0, 4 * gib},

		// 70% of a 512 MiB part is below the floor, and a card that small still has to be able to load one model.
		{"a tiny card hits the floor", 512 * mib, 1 * gib},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := deviceBudgetFor(tt.vramBytes); got != tt.want {
				t.Errorf("deviceBudgetFor(%d bytes) = %d, want %d", tt.vramBytes, got, tt.want)
			}
		})
	}
}

// TestBudgetOverride covers the support-triage knob, including that a typo in it doesn't take the app down.
func TestBudgetOverride(t *testing.T) {
	tests := []struct {
		name  string
		value string
		want  int64
		ok    bool
	}{
		{"a byte count is honoured", "1073741824", 1 << 30, true},
		{"zero means unbounded", "0", 0, true},
		{"a negative value is ignored", "-1", 0, false},
		{"gibberish is ignored", "lots", 0, false},
		{"a suffixed value is ignored", "4GiB", 0, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv(BudgetEnvVar, tt.value)

			got, ok := budgetOverride()
			if ok != tt.ok || got != tt.want {
				t.Errorf("budgetOverride() = (%d, %t), want (%d, %t)", got, ok, tt.want, tt.ok)
			}
		})
	}
}

// TestPoolOf checks that each provider is charged to the memory it actually occupies. Getting this wrong is what the
// two-pool split exists to prevent: charging a GPU-resident model against system RAM admits it onto a card that can't
// hold it, and the failure shows up only as a silent drop to the CPU.
func TestPoolOf(t *testing.T) {
	tests := []struct {
		ep   types.ExecutionProvider
		want types.MemoryPool
	}{
		{types.ExecutionProviderCUDA, types.MemoryPoolDevice},
		{types.ExecutionProviderTensorRT, types.MemoryPoolDevice},
		{types.ExecutionProviderDirectML, types.MemoryPoolDevice},
		{types.ExecutionProviderCPU, types.MemoryPoolHost},
		{types.ExecutionProviderOpenVINO, types.MemoryPoolHost},

		// Apple Silicon shares one physical pool between CPU and GPU, so there is no device budget to charge.
		{types.ExecutionProviderCoreML, types.MemoryPoolHost},
	}

	for _, tt := range tests {
		t.Run(string(tt.ep), func(t *testing.T) {
			if got := PoolOf(tt.ep); got != tt.want {
				t.Errorf("PoolOf(%s) = %s, want %s", tt.ep, got, tt.want)
			}
		})
	}

	// Auto is resolved inside the ONNX runtime and never reported back, so it is charged by platform.
	if runtime.GOOS == "darwin" {
		if got := PoolOf(types.ExecutionProviderAuto); got != types.MemoryPoolHost {
			t.Errorf("PoolOf(Auto) on darwin = %s, want host (unified memory)", got)
		}
	}
}
