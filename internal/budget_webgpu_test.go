package internal

import (
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// WebGPU is budgeted against host memory whatever the GPU: on the integrated GPUs it is aimed at there is no other
// memory, and charging it the device pool would apply the 1 GiB carve-out those report as VRAM.
func TestWebGPUIsBudgetedAgainstTheHostPool(t *testing.T) {
	if got := PoolOf(types.ExecutionProviderWebGPU); got != types.MemoryPoolHost {
		t.Fatalf("PoolOf(WebGPU) = %v, want %v", got, types.MemoryPoolHost)
	}
}
