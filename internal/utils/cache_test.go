package utils

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
)

// TestCleanVersionedCache covers the invalidation rule both the model cache and the NVIDIA library caches rely on: a
// stale stamp must clear the directory - files and subdirectories alike - while a matching one leaves it untouched.
func TestCleanVersionedCache(t *testing.T) {
	// setup redirects os.UserConfigDir at a fresh temp directory - HOME covers darwin, XDG_CONFIG_HOME linux and
	// AppData windows - and returns the cache directory CleanVersionedCache will resolve.
	setup := func(t *testing.T) string {
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

		return filepath.Join(dir, internal.AppName, "libs", "cudnn")
	}

	// populate fills the cache with the two shapes that must be removed: a plain file and a non-empty subdirectory.
	populate := func(t *testing.T, dir string) {
		t.Helper()

		if err := os.MkdirAll(filepath.Join(dir, "stale-dir"), 0o755); err != nil {
			t.Fatalf("failed to create the stale directory: %v", err)
		}
		if err := os.WriteFile(filepath.Join(dir, "stale-dir", "nested.so"), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to write the nested file: %v", err)
		}
		if err := os.WriteFile(filepath.Join(dir, "LICENSE.txt"), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to write the sentinel file: %v", err)
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

	t.Run("first run stamps an empty cache", func(t *testing.T) {
		dir := setup(t)

		wiped, err := CleanVersionedCache("libs/cudnn", "cudnn/9.23.1")
		if err != nil {
			t.Fatalf("CleanVersionedCache: %v", err)
		}
		if !wiped {
			t.Error("wiped = false, want true on a cache with no version file")
		}

		assertStamp(t, dir, "cudnn/9.23.1")
	})

	t.Run("an unstamped cache is wiped", func(t *testing.T) {
		dir := setup(t)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatalf("failed to create the cache directory: %v", err)
		}
		populate(t, dir)

		wiped, err := CleanVersionedCache("libs/cudnn", "cudnn/9.23.1")
		if err != nil {
			t.Fatalf("CleanVersionedCache: %v", err)
		}
		if !wiped {
			t.Error("wiped = false, want true")
		}

		assertEmpty(t, dir)
		assertStamp(t, dir, "cudnn/9.23.1")
	})

	t.Run("a stale stamp is wiped", func(t *testing.T) {
		dir := setup(t)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatalf("failed to create the cache directory: %v", err)
		}
		populate(t, dir)
		if err := os.WriteFile(filepath.Join(dir, ".version"), []byte("cudnn/9.0.0"), 0o644); err != nil {
			t.Fatalf("failed to write the version file: %v", err)
		}

		wiped, err := CleanVersionedCache("libs/cudnn", "cudnn/9.23.1")
		if err != nil {
			t.Fatalf("CleanVersionedCache: %v", err)
		}
		if !wiped {
			t.Error("wiped = false, want true")
		}

		if _, err = os.Stat(dir); err != nil {
			t.Errorf("the cache directory itself must survive the wipe: %v", err)
		}

		assertEmpty(t, dir)
		assertStamp(t, dir, "cudnn/9.23.1")
	})

	// The stamp is compared trimmed, so an editor that appended a newline doesn't force a multi-GB re-download.
	t.Run("a current stamp is a no-op", func(t *testing.T) {
		dir := setup(t)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatalf("failed to create the cache directory: %v", err)
		}
		populate(t, dir)
		if err := os.WriteFile(filepath.Join(dir, ".version"), []byte("cudnn/9.23.1\n"), 0o644); err != nil {
			t.Fatalf("failed to write the version file: %v", err)
		}

		wiped, err := CleanVersionedCache("libs/cudnn", "cudnn/9.23.1")
		if err != nil {
			t.Fatalf("CleanVersionedCache: %v", err)
		}
		if wiped {
			t.Error("wiped = true, want false on a current cache")
		}

		if _, err = os.Stat(filepath.Join(dir, "stale-dir", "nested.so")); err != nil {
			t.Errorf("a current cache must be left untouched: %v", err)
		}
		if _, err = os.Stat(filepath.Join(dir, "LICENSE.txt")); err != nil {
			t.Errorf("a current cache must be left untouched: %v", err)
		}
	})
}
