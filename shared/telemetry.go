package shared

import (
	"fmt"
	"log/slog"
	"maps"
	"slices"

	"github.com/vegidio/go-sak/o11y"
	"github.com/vegidio/go-sak/sysinfo"
	"github.com/vegidio/open-photo-ai/internal"
)

// ReportSystemInfo records the machine's CPU, memory and GPU details once per run, so a bug report carries the
// hardware it happened on without the user having to describe it.
func ReportSystemInfo(otel *o11y.Telemetry) {
	info := make(map[string]any)

	if cpu, err := sysinfo.GetCPUInfo(); err == nil {
		info["cpu.model"] = cpu.Name
		info["cpu.cores"] = cpu.Cores
	}

	// Through internal rather than sysinfo directly: these two probes shell out (`system_profiler` on macOS, a
	// PowerShell CIM query each on Windows) and internal memoizes them, so going straight to sysinfo would pay for both
	// a second time - on the blocking path before the window is created. Reported in bytes, because the raw fields are
	// two different units and were being emitted into one payload unlabelled.
	if total := internal.TotalRAMBytes(); total > 0 {
		info["memory"] = total
	}

	if gpu, err := internal.GPUInfo(); err == nil {
		for index, card := range gpu {
			key := fmt.Sprintf("gpu.%d.", index+1)
			info[key+"name"] = card.Name
			info[key+"memory"] = int64(card.Memory) << 20
		}
	}

	otel.LogInfo("System info", info)

	// Mirror the same CPU/memory/GPU info to the local log file. Reuse the map built above and emit
	// with sorted keys so the field order is deterministic and grouped (cpu.*, gpu.N.*, memory).
	attrs := make([]any, 0, len(info)*2)
	for _, k := range slices.Sorted(maps.Keys(info)) {
		attrs = append(attrs, k, info[k])
	}
	slog.Info("system info", attrs...)
}
