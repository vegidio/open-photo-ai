package internal

import "testing"

// TestEstimateModelBytes covers the pre-build size estimate the registry uses to free memory before an expensive
// session is built. The prefix match is what lets it sum a model split across several files.
func TestEstimateModelBytes(t *testing.T) {
	original := ModelData()
	t.Cleanup(func() { SetModelData(original) })

	SetModelData([]RemoteModelData{
		{Name: "up_kyoto_2x_fp32.onnx", Size: 200},
		{Name: "up_kyoto_4x_fp32.onnx", Size: 100},
		{Name: "up_osaka_fp16.onnx", Size: 10},
		{Name: "up_osaka_fp16.onnx.data", Size: 5000},
		{Name: "up_osaka_fp16_v2.onnx", Size: 90},
		{Name: "up_osaka_vae_decoder_fp16.onnx", Size: 700},
	})

	tests := []struct {
		name string
		id   string
		want int64
	}{
		{"single file model", "up_kyoto_4x_fp32", 100},
		{"external data is summed in", "up_osaka_fp16", 5010},
		{"unrelated prefix is excluded", "up_osaka_vae_decoder_fp16", 700},

		// Matching on the whole `<id>.onnx` stem rather than on the id alone is what keeps a versioned sibling out of
		// the set: on a bare prefix, up_osaka_fp16_v2.onnx would be charged to up_osaka_fp16 and downloaded with it.
		{"a versioned sibling is excluded", "up_osaka_fp16_v2", 90},

		// up_kyoto_8x_fp32 is run as the 4x model followed by the 2x one, so no manifest entry is named after it.
		// The registry must read this as "unknown" and fall back to charging the exact size after the build.
		{"composite operation is unknown", "up_kyoto_8x_fp32", 0},
		{"absent model is unknown", "dn_nowhere_fp32", 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := EstimateModelBytes(tt.id); got != tt.want {
				t.Errorf("EstimateModelBytes(%q) = %d, want %d", tt.id, got, tt.want)
			}
		})
	}
}

// An empty manifest is the normal outcome when LoadModelData times out, and must read as "unknown" rather than "free".
func TestEstimateModelBytesWithoutManifest(t *testing.T) {
	original := ModelData()
	t.Cleanup(func() { SetModelData(original) })

	SetModelData(nil)
	if got := EstimateModelBytes("up_kyoto_4x_fp32"); got != 0 {
		t.Errorf("EstimateModelBytes() = %d, want 0", got)
	}
}

// The three globals below are written by Initialize and read from download and inference goroutines, so they are
// atomics rather than plain variables. What matters for correctness is the swap: Destroy has to be able to take the
// cache away, not just close it, or a stray Process after teardown passes its non-nil guard and reaches a closed store.

func TestSwapImageCacheReturnsThePrevious(t *testing.T) {
	original := ImageCache()
	t.Cleanup(func() { SwapImageCache(original) })

	// Never dereferenced by the swap, so a bare pointer is enough to prove the handoff.
	first := &Cache{}
	second := &Cache{}

	SwapImageCache(nil)

	if previous := SwapImageCache(first); previous != nil {
		t.Errorf("swapping into an empty slot returned %p, want nil", previous)
	}

	previous := SwapImageCache(second)
	if previous != first {
		t.Errorf("swap returned %p, want the cache it replaced (%p) so the caller can close it", previous, first)
	}

	if got := ImageCache(); got != second {
		t.Errorf("ImageCache() = %p, want the newly installed %p", got, second)
	}
}

// This is the Destroy path: clearing the pointer is what makes the close safe, because Process guards on non-nil.
func TestSwapImageCacheClearsWithNil(t *testing.T) {
	original := ImageCache()
	t.Cleanup(func() { SwapImageCache(original) })

	SwapImageCache(&Cache{})
	SwapImageCache(nil)

	if got := ImageCache(); got != nil {
		t.Errorf("ImageCache() = %p after being cleared, want nil", got)
	}
}

func TestAppNameRoundTrips(t *testing.T) {
	original := AppName()
	t.Cleanup(func() { SetAppName(original) })

	SetAppName("opai-config-test")

	if got := AppName(); got != "opai-config-test" {
		t.Errorf("AppName() = %q, want %q", got, "opai-config-test")
	}
}

// The manifest is best-effort, and a nil one has to read back as nil rather than panicking in the accessor - it is the
// normal outcome of a fetch that timed out, and ModelFiles walks it on every model download.
func TestModelDataHandlesNil(t *testing.T) {
	original := ModelData()
	t.Cleanup(func() { SetModelData(original) })

	SetModelData(nil)

	if got := ModelData(); got != nil {
		t.Errorf("ModelData() = %v, want nil", got)
	}

	if got := ModelFiles("anything"); len(got) != 0 {
		t.Errorf("ModelFiles on a nil manifest returned %d entries, want none", len(got))
	}
}
