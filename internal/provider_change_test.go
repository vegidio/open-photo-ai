package internal

import (
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// acquireProvider acquires id on ep, so a test can build entries on different providers.
func acquireProvider(id string, ep types.ExecutionProvider) (*Lease, error) {
	return AcquireModel(id, ep, func(types.ExecutionProvider) (any, error) {
		return &fakeModel{bytes: 100}, nil
	})
}

// A processor change unloads what was built on the old one and keeps what the new one would use. Holding the old
// processor's models for their full idle TTL is what let two 7 GB sessions sit on one card at once.
func TestDrainOtherProvidersUnloadsOnlyTheStaleProviders(t *testing.T) {
	withRegistry(t)

	stale, err := acquireProvider("stale", types.ExecutionProviderCUDA)
	if err != nil {
		t.Fatalf("acquire stale: %v", err)
	}
	stale.Release()

	kept, err := acquireProvider("kept", types.ExecutionProviderTensorRT)
	if err != nil {
		t.Fatalf("acquire kept: %v", err)
	}
	kept.Release()

	if got := Registry.Len(); got != 2 {
		t.Fatalf("want 2 resident models before the switch, got %d", got)
	}

	DestroyEntries(Registry.DrainOtherProviders(types.ExecutionProviderTensorRT))

	if _, ok := Registry.entries[registryKey("stale", types.ExecutionProviderCUDA)]; ok {
		t.Error("the model built on the old processor is still resident")
	}

	if _, ok := Registry.entries[registryKey("kept", types.ExecutionProviderTensorRT)]; !ok {
		t.Error("a model built on the newly chosen processor was needlessly unloaded")
	}
}

// A model still running is removed from the registry but not destroyed under its user, so switching mid-export is
// safe. It is the same contract DrainAll keeps.
func TestDrainOtherProvidersLeavesModelsInUseToTheirLastRelease(t *testing.T) {
	withRegistry(t)

	lease, err := acquireProvider("running", types.ExecutionProviderCUDA)
	if err != nil {
		t.Fatalf("acquire running: %v", err)
	}

	victims := Registry.DrainOtherProviders(types.ExecutionProviderTensorRT)
	if len(victims) != 0 {
		t.Fatalf("a leased model must not be handed back for destruction, got %d", len(victims))
	}

	if _, ok := Registry.entries[registryKey("running", types.ExecutionProviderCUDA)]; ok {
		t.Error("the entry should already be unreachable so the next acquire builds fresh")
	}

	// The model is still usable by its holder, and is destroyed when that holder lets go.
	if lease.Model() == nil {
		t.Error("the leased model was freed under its user")
	}

	lease.Release()
}
