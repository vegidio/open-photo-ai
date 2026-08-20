package deps

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
)

// TestPruneLegacyRuntime covers the sweep of the runtime older versions extracted into the config root. What it must
// not touch matters more than what it removes: the models, caches and logs are siblings of those files.
func TestPruneLegacyRuntime(t *testing.T) {
	root := setup(t)

	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("failed to create the config directory: %v", err)
	}

	stale := []string{
		"onnxruntime.1.26.0.dylib",
		"libonnxruntime_providers_cuda.so",
		"onnxruntime-1.26.0.dll",
		"LICENSE.txt",
		"VERSION_NUMBER",
	}
	for _, name := range stale {
		if err := os.WriteFile(filepath.Join(root, name), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to write %s: %v", name, err)
		}
	}

	// A file the sweep must leave alone, so the globs can't quietly grow into user data.
	if err := os.WriteFile(filepath.Join(root, "settings.json"), []byte("{}"), 0o644); err != nil {
		t.Fatalf("failed to write the settings file: %v", err)
	}

	for _, dir := range []string{internal.ModelsDir, internal.EngineCacheDir, "cache", "logs", "libs"} {
		if err := os.MkdirAll(filepath.Join(root, dir), 0o755); err != nil {
			t.Fatalf("failed to create %s: %v", dir, err)
		}
	}

	removed, err := PruneLegacyRuntime()
	if err != nil {
		t.Fatalf("PruneLegacyRuntime: %v", err)
	}
	if removed != len(stale) {
		t.Errorf("removed = %d, want %d", removed, len(stale))
	}

	for _, name := range stale {
		if _, statErr := os.Stat(filepath.Join(root, name)); !os.IsNotExist(statErr) {
			t.Errorf("%s survived the sweep", name)
		}
	}

	if _, err = os.Stat(filepath.Join(root, "settings.json")); err != nil {
		t.Errorf("an unrelated file must survive: %v", err)
	}
	for _, dir := range []string{internal.ModelsDir, internal.EngineCacheDir, "cache", "logs", "libs"} {
		if _, err = os.Stat(filepath.Join(root, dir)); err != nil {
			t.Errorf("%s must survive: %v", dir, err)
		}
	}

	// Idempotent, because it runs on every start rather than behind a marker of its own.
	if removed, err = PruneLegacyRuntime(); err != nil || removed != 0 {
		t.Errorf("second sweep removed %d files (err %v), want 0", removed, err)
	}
}

// TestPruneLegacyEPCache covers the sweep of the provider caches that used to share the models directory. Install
// cannot do this itself: models/ is shared by every model, so it only ever touches the paths its own manifest names,
// and these files were named by nobody.
// The sweep is a one-time migration, so it must stop looking once it has run. Its globs describe the old layout but
// would also match a `*.dll` or `README` a later feature legitimately puts at the config root.
func TestPruneLegacyRuntimeRunsOnlyOnce(t *testing.T) {
	root := setup(t)

	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("failed to create the config directory: %v", err)
	}

	if err := os.WriteFile(filepath.Join(root, "onnxruntime.1.26.0.dylib"), []byte("x"), 0o644); err != nil {
		t.Fatalf("failed to write the stale runtime: %v", err)
	}

	removed, err := PruneLegacyRuntime()
	if err != nil {
		t.Fatalf("PruneLegacyRuntime: %v", err)
	}
	if removed != 1 {
		t.Fatalf("first sweep removed %d files, want 1", removed)
	}

	// A file a later feature might legitimately place at the config root, matching the same globs.
	later := filepath.Join(root, "README.md")
	if err = os.WriteFile(later, []byte("docs"), 0o644); err != nil {
		t.Fatalf("failed to write the later file: %v", err)
	}

	if removed, err = PruneLegacyRuntime(); err != nil {
		t.Fatalf("second PruneLegacyRuntime: %v", err)
	}
	if removed != 0 {
		t.Fatalf("second sweep removed %d files, want 0", removed)
	}

	if _, err = os.Stat(later); err != nil {
		t.Errorf("the second sweep removed a file written after the migration had already run: %v", err)
	}
}

func TestPruneLegacyEPCache(t *testing.T) {
	seed := func(t *testing.T, dir string, stamped bool) {
		t.Helper()

		if err := os.MkdirAll(filepath.Join(dir, "13892371"), 0o755); err != nil {
			t.Fatalf("failed to create the compiled model directory: %v", err)
		}
		if err := os.WriteFile(filepath.Join(dir, "13892371", "model.mlmodelc"), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to write the compiled model: %v", err)
		}

		files := map[string]string{
			"dn_stockholm_fp32.onnx":  "model",
			"up_osaka_fp16.onnx.data": "weights",
			".up_kyoto_4x_fp32.json":  "{}",
			"TensorrtExecutionProvider_TRTKernel_graph_x_0_0_deadbeef.engine":  "engine",
			"TensorrtExecutionProvider_TRTKernel_graph_x_0_0_deadbeef.profile": "profile",
		}
		for name, body := range files {
			if err := os.WriteFile(filepath.Join(dir, name), []byte(body), 0o644); err != nil {
				t.Fatalf("failed to write %s: %v", name, err)
			}
		}

		if stamped {
			if err := os.WriteFile(filepath.Join(dir, ".version"), []byte("runtime/1.26.0"), 0o644); err != nil {
				t.Fatalf("failed to write the stamp: %v", err)
			}
		}
	}

	t.Run("a stamped models directory is swept once", func(t *testing.T) {
		root := setup(t)
		dir := filepath.Join(root, internal.ModelsDir)
		seed(t, dir, true)

		removed, err := PruneLegacyEPCache()
		if err != nil {
			t.Fatalf("PruneLegacyEPCache: %v", err)
		}
		if removed != 3 {
			t.Errorf("removed = %d, want 3 (two TensorRT files and one compiled model directory)", removed)
		}

		for _, name := range []string{"dn_stockholm_fp32.onnx", "up_osaka_fp16.onnx.data", ".up_kyoto_4x_fp32.json"} {
			if _, statErr := os.Stat(filepath.Join(dir, name)); statErr != nil {
				t.Errorf("%s must survive the sweep: %v", name, statErr)
			}
		}
		for _, name := range []string{
			"13892371",
			"TensorrtExecutionProvider_TRTKernel_graph_x_0_0_deadbeef.engine",
			"TensorrtExecutionProvider_TRTKernel_graph_x_0_0_deadbeef.profile",
		} {
			if _, statErr := os.Stat(filepath.Join(dir, name)); !os.IsNotExist(statErr) {
				t.Errorf("%s survived the sweep", name)
			}
		}

		// The stamp is the marker as well as the trigger, so removing it is what makes this run exactly once.
		if _, err = os.Stat(filepath.Join(dir, ".version")); !os.IsNotExist(err) {
			t.Error("the stamp must be removed so the sweep does not run again")
		}
		if removed, err = PruneLegacyEPCache(); err != nil || removed != 0 {
			t.Errorf("second sweep removed %d entries (err %v), want 0", removed, err)
		}
	})

	// A fresh installation never had a stamp, and its models directory must not be touched at all - the engine cache
	// lives elsewhere now, so anything unrecognised there belongs to someone else.
	t.Run("an unstamped models directory is left alone", func(t *testing.T) {
		root := setup(t)
		dir := filepath.Join(root, internal.ModelsDir)
		seed(t, dir, false)

		removed, err := PruneLegacyEPCache()
		if err != nil {
			t.Fatalf("PruneLegacyEPCache: %v", err)
		}
		if removed != 0 {
			t.Errorf("removed = %d, want 0", removed)
		}

		if _, err = os.Stat(filepath.Join(dir, "13892371")); err != nil {
			t.Errorf("an unstamped directory must be left untouched: %v", err)
		}
	})
}
