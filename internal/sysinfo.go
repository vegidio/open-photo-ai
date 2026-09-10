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
// The warning is logged inside the memoized body rather than at each call site. Four callers - the CUDA and TensorRT
// probes, the device budget, the telemetry header - all turn this error into a bare false or a zero, so the reason the
// machine looked like it had no GPU ("why is CUDA not being offered?") was discarded four separate times and never
// printed anywhere. Being inside OnceValues is what keeps a repeated call from repeating the line.
//
// It lives here rather than in the public utils package so that the budget code, which utils imports, can share the
// one cache instead of starting a second.
// It stays a var rather than becoming a func so tests can swap it - see stubGPUInfo in sysinfo_test.go.
var GPUInfo = sync.OnceValues(func() ([]sysinfo.GPUInfo, error) {
	gpus, err := sysinfo.GetGPUInfo()
	if err != nil {
		Log().Warn("could not query the GPUs; CUDA and TensorRT will not be offered", "err", err)
	}

	return gpus, err
})

// MemoryInfo returns the machine's total physical RAM, querying the OS at most once per process. Same reasoning as
// GPUInfo: on Windows it is a PowerShell CIM query.
//
// The warning is logged inside the memoized body for the same reason as GPUInfo: TotalRAMBytes collapses this to 0 and
// the budget code then warns about the default it picked, without ever saying what the OS actually said.
//
// Prefer TotalRAMBytes over reading .Total from this directly: it collapses a failed probe to 0, which is the
// answer the budget code is written against.
// Stays a var for the same reason as GPUInfo: tests replace it.
var MemoryInfo = sync.OnceValues(func() (sysinfo.MemoryInfo, error) {
	info, err := sysinfo.GetMemoryInfo()
	if err != nil {
		Log().Warn("could not query the system memory; the default host budget will be used", "err", err)
	}

	return info, err
})

// bytesPerReportedVramUnit converts what sysinfo.GPUInfo reports for GPU memory into bytes: mebibytes.
//
// It is pinned here rather than at each call site because sysinfo reports its two memory figures in two different
// units - GPU memory in mebibytes, system RAM in bytes - and nothing in either field's name says so.
const bytesPerReportedVramUnit = int64(1) << 20

// TotalRAMBytes returns the machine's total physical RAM in bytes, or 0 when it can't be queried.
//
// sysinfo.MemoryInfo.Total is already bytes and needs no scaling. It has not always been: go-sak before 26.5.0
// documented the field as bytes while every backend returned decimal megabytes, so this used to multiply by
// 1,000,000 to compensate. The go-sak bump in 26.9.0 fixed the field and left the compensation behind, which is
// worth spelling out because neither symptom points at this line. Telemetry reported a 59 GiB machine as 63 PB; and
// hostBudgetFor clamps to maxHostBudget, so every machine whose probe succeeded - an 8 GB laptop included - silently
// got the maximum host budget instead of one sized to its RAM. Do not reintroduce a scale factor here without first
// checking what the go-sak backends actually return.
func TotalRAMBytes() int64 {
	info, err := MemoryInfo()
	if err != nil {
		return 0
	}

	return int64(info.Total)
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
