package internal

import (
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// A device session costs more than its weights, and the budget has to know it: the allowance is what keeps the
// registry from admitting more than the card holds, which on Windows does not fail but silently pages to host RAM.
//
// The host half is the CPU-fallback case too. Both callers derive the pool with PoolOf from the provider the model was
// actually built on, and PoolOf(CPU) is the host pool - so a model that failed on CUDA and rebuilt on the CPU arrives
// here already charged against the host and never sees the allowance.
func TestChargedBytesAddsTheDeviceAllowance(t *testing.T) {
	t.Setenv(OverheadEnvVar, "50")

	if got := chargedBytes(types.MemoryPoolDevice, 1000); got != 1500 {
		t.Errorf("device charge = %d, want 1500", got)
	}

	// The host pool sizes system RAM directly and gets no allowance.
	if got := chargedBytes(types.MemoryPoolHost, 1000); got != 1000 {
		t.Errorf("host charge = %d, want 1000", got)
	}
}

// Zero means "unknown", not "free" - EstimateModelBytes returns it for anything the manifest can't name - so it must
// not grow an allowance on a size nobody has.
func TestChargedBytesLeavesUnknownSizesAlone(t *testing.T) {
	t.Setenv(OverheadEnvVar, "50")

	if got := chargedBytes(types.MemoryPoolDevice, 0); got != 0 {
		t.Errorf("unknown charge = %d, want 0", got)
	}
}

func TestOverheadOverrideIsHonouredAndValidated(t *testing.T) {
	t.Setenv(OverheadEnvVar, "0")
	if got := chargedBytes(types.MemoryPoolDevice, 1000); got != 1000 {
		t.Errorf("charge with the allowance disabled = %d, want 1000", got)
	}

	// A nonsense value falls back to the built-in rather than failing or charging something absurd.
	t.Setenv(OverheadEnvVar, "not-a-number")
	if got := overheadPercent(); got != deviceOverheadPercent {
		t.Errorf("overhead = %d, want the default %d", got, deviceOverheadPercent)
	}
}
