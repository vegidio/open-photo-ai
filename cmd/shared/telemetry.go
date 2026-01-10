package shared

import (
	"fmt"

	"github.com/vegidio/go-sak/o11y"
	"github.com/vegidio/go-sak/sysinfo"
)

func ReportSystemInfo(tel *o11y.Telemetry) {
	info := make(map[string]any)

	if cpu, err := sysinfo.GetCPUInfo(); err == nil {
		info["cpu.model"] = cpu.Name
		info["cpu.cores"] = cpu.Cores
	}

	if mem, err := sysinfo.GetMemoryInfo(); err == nil {
		info["memory"] = mem.Total
	}

	if gpu, err := sysinfo.GetGPUInfo(); err == nil {
		for index, card := range gpu {
			key := fmt.Sprintf("gpu.%d.", index+1)
			info[key+"name"] = card.Name
			info[key+"memory"] = card.Memory
		}
	}

	tel.LogInfo("System info", info)
}
