package internal

import "testing"

// TestEstimateModelBytes covers the pre-build size estimate the registry uses to free memory before an expensive
// session is built. The prefix match is what lets it sum a model split across several files.
func TestEstimateModelBytes(t *testing.T) {
	original := ModelData
	t.Cleanup(func() { ModelData = original })

	ModelData = []RemoteModelData{
		{Name: "up_kyoto_2x_fp32.onnx", Size: 200},
		{Name: "up_kyoto_4x_fp32.onnx", Size: 100},
		{Name: "up_osaka_fp16.onnx", Size: 10},
		{Name: "up_osaka_fp16.onnx.data", Size: 5000},
		{Name: "up_osaka_fp16_v2.onnx", Size: 90},
		{Name: "up_osaka_vae_decoder_fp16.onnx", Size: 700},
	}

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
	original := ModelData
	t.Cleanup(func() { ModelData = original })

	ModelData = nil
	if got := EstimateModelBytes("up_kyoto_4x_fp32"); got != 0 {
		t.Errorf("EstimateModelBytes() = %d, want 0", got)
	}
}
