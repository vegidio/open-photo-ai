package internal

import (
	"sync"

	"github.com/vegidio/go-sak/sysinfo"
)

// GPUInfo returns the machine's GPUs, querying the OS at most once per process.
//
// sysinfo.GetGPUInfo shells out - `system_profiler SPDisplaysDataType` on macOS, a PowerShell CIM query on Windows,
// `lspci` plus `nvidia-smi` on Linux - and takes a second or more on some machines. Several callers want the answer
// (which execution providers are available, how large the device memory budget should be, what the benchmark header
// prints), and it cannot change while the process is alive short of hot-plugging an eGPU.
//
// It lives here rather than in the public utils package so that the budget code, which utils imports, can share the
// one cache instead of starting a second.
var GPUInfo = sync.OnceValues(sysinfo.GetGPUInfo)

// MemoryInfo returns the machine's total physical RAM, querying the OS at most once per process. Same reasoning as
// GPUInfo: on Windows it is a PowerShell CIM query.
//
// Prefer TotalRAMBytes over reading .Total from this directly - see the unit note there.
var MemoryInfo = sync.OnceValues(sysinfo.GetMemoryInfo)

const (
	// bytesPerReportedMB converts what sysinfo.MemoryInfo actually returns into bytes.
	//
	// Its field is documented as bytes, but every platform backend divides by 1,000,000 before returning - decimal
	// megabytes, not bytes and not mebibytes. Taking the doc comment at its word makes a 64 GB machine look like 68 KB
	// of RAM. Verified against the darwin (`sysctl hw.memsize`), linux (`/proc/meminfo`) and windows (CIM) paths.
	bytesPerReportedMB = int64(1_000_000)

	// bytesPerReportedVramUnit converts what sysinfo.GPUInfo reports for GPU memory into bytes. It is mebibytes - a
	// different unit from the one the same package uses for system RAM, which is the whole reason both conversions are
	// pinned here rather than at each call site.
	bytesPerReportedVramUnit = int64(1) << 20
)

// TotalRAMBytes returns the machine's total physical RAM in bytes, or 0 when it can't be queried.
//
// This is the only place the decimal-megabytes quirk above is compensated for, so no consumer can read the raw field
// and get the unit wrong.
func TotalRAMBytes() int64 {
	info, err := MemoryInfo()
	if err != nil {
		return 0
	}

	return int64(info.Total) * bytesPerReportedMB
}

// LargestVRAMBytes returns the memory of the machine's largest GPU, in bytes, and whether any GPU reported a figure at
// all.
//
// The largest, not the sum: a model is built on one device, so what matters is the card it will land on rather than
// the machine's total. A card that reports 0 is either integrated or unqueryable - go-sak deliberately reports 0 on
// the Windows CIM path, because AdapterRAM is unreliable on modern cards - so "none reported" is a real answer that
// callers have to handle rather than an error.
func LargestVRAMBytes() (bytes int64, ok bool) {
	gpus, err := GPUInfo()
	if err != nil {
		return 0, false
	}

	var largest int64
	for _, gpu := range gpus {
		largest = max(largest, int64(gpu.Memory)*bytesPerReportedVramUnit)
	}

	return largest, largest > 0
}
