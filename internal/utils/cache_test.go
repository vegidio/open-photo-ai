package utils

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
)

// setupConfigRoot redirects os.UserConfigDir at a fresh temp directory - HOME covers darwin, XDG_CONFIG_HOME linux and
// AppData windows - and returns the application's directory inside it.
func setupConfigRoot(t *testing.T) string {
	t.Helper()

	root := t.TempDir()
	t.Setenv("HOME", root)
	t.Setenv("XDG_CONFIG_HOME", root)
	t.Setenv("AppData", root)

	internal.AppName = "opai-test"

	dir, err := os.UserConfigDir()
	if err != nil {
		t.Fatalf("failed to resolve the user config directory: %v", err)
	}

	return filepath.Join(dir, internal.AppName)
}

// TestCleanEPCache covers the invalidation the compiled execution provider caches rely on: a stale stamp must clear the
// directory, a matching one must leave it alone, and the models must survive either way.
//
// That last part is the behaviour being fixed. The cache used to live in the models directory, so invalidating it on a
// runtime bump meant deleting every downloaded model - several gigabytes - to throw away engines that were the only
// thing actually tied to the runtime version.
func TestCleanEPCache(t *testing.T) {
	setup := func(t *testing.T) (engines, models string) {
		t.Helper()

		root := setupConfigRoot(t)
		return filepath.Join(root, internal.EngineCacheDir), filepath.Join(root, internal.ModelsDir)
	}

	// populate fills the cache with what the providers actually write: TensorRT drops loose engine and profile files,
	// CoreML a compiled model directory.
	populate := func(t *testing.T, dir string) {
		t.Helper()

		if err := os.MkdirAll(filepath.Join(dir, "dn_stockholm_fp32", "13892371"), 0o755); err != nil {
			t.Fatalf("failed to create the compiled model directory: %v", err)
		}
		if err := os.WriteFile(
			filepath.Join(dir, "dn_stockholm_fp32", "13892371", "model.mlmodelc"), []byte("x"), 0o644,
		); err != nil {
			t.Fatalf("failed to write the compiled model: %v", err)
		}
		if err := os.WriteFile(
			filepath.Join(dir, "TensorrtExecutionProvider_TRTKernel_graph_x_0_0_deadbeef.engine"), []byte("x"), 0o644,
		); err != nil {
			t.Fatalf("failed to write the engine file: %v", err)
		}
	}

	seedModels := func(t *testing.T, dir string) {
		t.Helper()

		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatalf("failed to create the models directory: %v", err)
		}
		for _, name := range []string{"dn_stockholm_fp32.onnx", "up_osaka_fp16.onnx.data", ".up_kyoto_4x_fp32.json"} {
			if err := os.WriteFile(filepath.Join(dir, name), []byte("x"), 0o644); err != nil {
				t.Fatalf("failed to write %s: %v", name, err)
			}
		}
	}

	assertStamp := func(t *testing.T, dir, want string) {
		t.Helper()

		got, err := os.ReadFile(filepath.Join(dir, ".version"))
		if err != nil {
			t.Fatalf("failed to read the version file: %v", err)
		}
		if string(got) != want {
			t.Errorf("version file = %q, want %q", got, want)
		}
	}

	assertEmpty := func(t *testing.T, dir string) {
		t.Helper()

		entries, err := os.ReadDir(dir)
		if err != nil {
			t.Fatalf("failed to read the cache directory: %v", err)
		}
		for _, e := range entries {
			if e.Name() != ".version" {
				t.Errorf("%s survived the wipe", e.Name())
			}
		}
	}

	assertModelsIntact := func(t *testing.T, dir string) {
		t.Helper()

		for _, name := range []string{"dn_stockholm_fp32.onnx", "up_osaka_fp16.onnx.data", ".up_kyoto_4x_fp32.json"} {
			if _, err := os.Stat(filepath.Join(dir, name)); err != nil {
				t.Errorf("invalidating the engine cache must not touch %s: %v", name, err)
			}
		}
	}

	t.Run("first run stamps an empty cache", func(t *testing.T) {
		engines, _ := setup(t)

		wiped, err := CleanEPCache("runtime/1.26.0")
		if err != nil {
			t.Fatalf("CleanEPCache: %v", err)
		}
		if !wiped {
			t.Error("wiped = false, want true on a cache with no version file")
		}

		assertStamp(t, engines, "runtime/1.26.0")
	})

	t.Run("a stale stamp wipes engines and leaves the models", func(t *testing.T) {
		engines, models := setup(t)
		if err := os.MkdirAll(engines, 0o755); err != nil {
			t.Fatalf("failed to create the cache directory: %v", err)
		}
		populate(t, engines)
		seedModels(t, models)
		if err := os.WriteFile(filepath.Join(engines, ".version"), []byte("runtime/1.25.0"), 0o644); err != nil {
			t.Fatalf("failed to write the version file: %v", err)
		}

		wiped, err := CleanEPCache("runtime/1.26.0")
		if err != nil {
			t.Fatalf("CleanEPCache: %v", err)
		}
		if !wiped {
			t.Error("wiped = false, want true")
		}

		if _, err = os.Stat(engines); err != nil {
			t.Errorf("the cache directory itself must survive the wipe: %v", err)
		}

		assertEmpty(t, engines)
		assertStamp(t, engines, "runtime/1.26.0")
		assertModelsIntact(t, models)
	})

	// The stamp is compared trimmed, so an editor that appended a newline doesn't force every engine to be rebuilt.
	t.Run("a current stamp is a no-op", func(t *testing.T) {
		engines, _ := setup(t)
		if err := os.MkdirAll(engines, 0o755); err != nil {
			t.Fatalf("failed to create the cache directory: %v", err)
		}
		populate(t, engines)
		if err := os.WriteFile(filepath.Join(engines, ".version"), []byte("runtime/1.26.0\n"), 0o644); err != nil {
			t.Fatalf("failed to write the version file: %v", err)
		}

		wiped, err := CleanEPCache("runtime/1.26.0")
		if err != nil {
			t.Fatalf("CleanEPCache: %v", err)
		}
		if wiped {
			t.Error("wiped = true, want false on a current cache")
		}

		engine := filepath.Join(engines, "TensorrtExecutionProvider_TRTKernel_graph_x_0_0_deadbeef.engine")
		if _, err = os.Stat(engine); err != nil {
			t.Errorf("a current cache must be left untouched: %v", err)
		}
	})
}
