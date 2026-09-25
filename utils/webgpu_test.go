package utils

import (
	"os"
	"path/filepath"
	"slices"
	"testing"
)

// Without the DirectX shader compiler beside it the Windows plugin registers and lists its adapters, then fails every
// device request, so its absence has to be caught at install rather than at every session build.
func TestMissingWebGPUCompanionsOnWindows(t *testing.T) {
	dir := t.TempDir()

	if got := missingWebGPUCompanions(dir, "windows"); !slices.Equal(got, []string{"dxil.dll", "dxcompiler.dll"}) {
		t.Errorf("empty dir: missing = %v, want both shader compiler libraries", got)
	}

	for _, name := range []string{"dxil.dll", "dxcompiler.dll"} {
		if err := os.WriteFile(filepath.Join(dir, name), nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}

	if got := missingWebGPUCompanions(dir, "windows"); len(got) != 0 {
		t.Errorf("complete dir: missing = %v, want none", got)
	}
}

// Vulkan and Metal compile shaders in the driver, so an archive holding only the plugin is complete there.
func TestMissingWebGPUCompanionsElsewhere(t *testing.T) {
	for _, goos := range []string{"linux", "darwin"} {
		if got := missingWebGPUCompanions(t.TempDir(), goos); len(got) != 0 {
			t.Errorf("%s: missing = %v, want none", goos, got)
		}
	}
}
