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
var MemoryInfo = sync.OnceValues(sysinfo.GetMemoryInfo)
