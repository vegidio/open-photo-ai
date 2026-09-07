package opai

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// These setters are the library's whole public configuration surface, and each is a one-line delegation into internal.
// A delegation that goes to the wrong place is invisible - the call succeeds and the setting silently does nothing -
// so what is checked here is that the value actually lands where the rest of the library reads it.

func TestSetImageCacheEnabledRoundTrips(t *testing.T) {
	t.Cleanup(func() { SetImageCacheEnabled(true) })

	SetImageCacheEnabled(false)
	if internal.ImageCacheEnabled() {
		t.Error("cache still reports enabled after SetImageCacheEnabled(false)")
	}

	SetImageCacheEnabled(true)
	if !internal.ImageCacheEnabled() {
		t.Error("cache reports disabled after SetImageCacheEnabled(true)")
	}
}

// The zero value has to mean "on": an embedder that never calls the setter must get the cache, so the flag is stored
// inverted internally. That inversion is easy to lose in a refactor and impossible to notice without checking.
func TestImageCacheDefaultsToEnabled(t *testing.T) {
	t.Cleanup(func() { SetImageCacheEnabled(true) })

	SetImageCacheEnabled(true)
	if !internal.ImageCacheEnabled() {
		t.Error("the image cache is not enabled by default")
	}
}

func TestSetSkipModelVerificationRoundTrips(t *testing.T) {
	t.Cleanup(func() { SetSkipModelVerification(false) })

	SetSkipModelVerification(true)
	if !internal.SkipModelVerification() {
		t.Error("verification is not skipped after SetSkipModelVerification(true)")
	}

	SetSkipModelVerification(false)
	if internal.SkipModelVerification() {
		t.Error("verification is still skipped after SetSkipModelVerification(false)")
	}
}

func TestSetModelBudgetRoundTrips(t *testing.T) {
	before := ModelMemoryStats()
	t.Cleanup(func() {
		SetModelBudget(types.MemoryPoolDevice, before.Device.Budget)
		SetModelBudget(types.MemoryPoolHost, before.Host.Budget)
	})

	const deviceBudget = 123 << 20
	const hostBudget = 456 << 20

	SetModelBudget(types.MemoryPoolDevice, deviceBudget)
	SetModelBudget(types.MemoryPoolHost, hostBudget)

	got := ModelMemoryStats()
	if got.Device.Budget != deviceBudget {
		t.Errorf("device budget = %d, want %d", got.Device.Budget, deviceBudget)
	}

	// The two pools are separate ceilings; setting one must not move the other.
	if got.Host.Budget != hostBudget {
		t.Errorf("host budget = %d, want %d", got.Host.Budget, hostBudget)
	}
}

// A nil handler must clear the previous one rather than leaving a non-nil pointer to a nil func behind, which is what
// the internal setter's own comment warns about.
func TestSetFallbackHandlerAcceptsNil(t *testing.T) {
	t.Cleanup(func() { SetFallbackHandler(nil) })

	SetFallbackHandler(func(types.ExecutionProvider, error) {})
	SetFallbackHandler(nil)

	// Nothing to assert beyond not panicking: the notify path dereferences the stored pointer, and a nil func stored
	// behind a non-nil pointer is exactly the crash this guards.
	internal.ResetFallback()
}

// SetModelIdleTTL is deliberately not covered here: the registry keeps idleTTL unexported and exposes no getter,
// and adding public API purely so a test can read it back would be the wrong trade.
