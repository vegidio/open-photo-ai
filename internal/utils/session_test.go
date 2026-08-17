package utils

import (
	"os"
	"path/filepath"
	"testing"
)

// TestModelFileBytes covers the sizing rule the model registry budgets against. The case that matters is the
// external-data one: a model whose weights live in a sibling file must be charged for both, or a 7 GB model looks
// like a 7 MB one.
func TestModelFileBytes(t *testing.T) {
	dir := t.TempDir()

	write := func(name string, size int) {
		t.Helper()
		if err := os.WriteFile(filepath.Join(dir, name), make([]byte, size), 0o644); err != nil {
			t.Fatalf("failed to write %s: %v", name, err)
		}
	}

	// A self-contained model, plus a second one whose name it is NOT a prefix of.
	write("up_kyoto_4x_fp32.onnx", 100)
	write("up_kyoto_2x_fp32.onnx", 200)

	// A model split across a graph and an external-data blob, the shape up_osaka_fp16 has on the model repo.
	write("up_osaka_fp16.onnx", 10)
	write("up_osaka_fp16.onnx.data", 5000)

	// The other two external-data conventions: appended to the full name, and replacing the extension.
	write("dn_underscore_fp32.onnx", 3)
	write("dn_underscore_fp32.onnx_data", 40)
	write("dn_stem_fp32.onnx", 7)
	write("dn_stem_fp32.data", 800)

	// A sibling that merely shares a stem must not be swept in: up_osaka_vae_decoder_fp16 is its own model.
	write("up_osaka_vae_decoder_fp16.onnx", 700)

	// An extension-less model makes the stem-derived candidate collide with the `.data` one. Only one of them may be
	// counted, or the blob is charged twice.
	write("dn_noext_fp32", 9)
	write("dn_noext_fp32.data", 60)

	// The models directory also holds the TensorRT engine cache; a directory must never be counted.
	if err := os.Mkdir(filepath.Join(dir, "up_kyoto_4x_fp32.onnx_cache"), 0o755); err != nil {
		t.Fatalf("failed to create cache dir: %v", err)
	}

	tests := []struct {
		name string
		file string
		want int64
	}{
		{"self-contained model", "up_kyoto_4x_fp32.onnx", 100},
		{"external data is included", "up_osaka_fp16.onnx", 5010},
		{"underscore convention is included", "dn_underscore_fp32.onnx", 43},
		{"stem convention is included", "dn_stem_fp32.onnx", 807},
		{"unrelated prefix is excluded", "up_osaka_vae_decoder_fp16.onnx", 700},
		{"extension-less model charges its blob once", "dn_noext_fp32", 69},
		{"missing model sizes to zero", "does_not_exist.onnx", 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := modelFileBytes(filepath.Join(dir, tt.file)); got != tt.want {
				t.Errorf("modelFileBytes(%s) = %d, want %d", tt.file, got, tt.want)
			}
		})
	}
}

func TestSessionsResidentBytes(t *testing.T) {
	// A nil session contributes 0 rather than panicking: an upscale pass that failed to load must not take down the
	// size accounting with it.
	if got := Sessions(nil).ResidentBytes(); got != 0 {
		t.Errorf("Sessions(nil).ResidentBytes() = %d, want 0", got)
	}

	sessions := Sessions{{bytes: 10}, {bytes: 32}, nil}
	if got := sessions.ResidentBytes(); got != 42 {
		t.Errorf("Sessions.ResidentBytes() = %d, want 42", got)
	}
}
