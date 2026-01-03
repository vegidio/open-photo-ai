package shared

import (
	"fmt"

	"github.com/jaypipes/ghw"
	"github.com/vegidio/go-sak/o11y"
)

func ReportSystemInfo(tel *o11y.Telemetry) {
	info := make(map[string]any)

	if cpu, err := ghw.CPU(); err == nil {
		for index, processor := range cpu.Processors {
			key := fmt.Sprintf("cpu.%d.", index+1)
			info[key+"model"] = processor.Model
			info[key+"cores"] = len(processor.Cores)
		}
	}

	if mem, err := ghw.Memory(); err == nil {
		info["memory"] = mem.TotalPhysicalBytes
	}

	if gpu, err := ghw.GPU(); err == nil {
		for index, card := range gpu.GraphicsCards {
			key := fmt.Sprintf("gpu.%d.", index+1)

			if card.DeviceInfo != nil && card.DeviceInfo.Product != nil {
				info[key+"name"] = card.DeviceInfo.Product.Name
			}

			if card.Node != nil {
				info[key+"memory"] = card.Node.Memory.TotalPhysicalBytes
			}
		}
	}

	tel.LogInfo("System info", info)
}
