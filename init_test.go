package opai

import (
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// withTempImageCache points the config directory at one of this test's own and guarantees the store this test opens is
// closed afterwards. Without the cleanup a test would leave Badger holding the directory lock for the rest of the run,
// and every test after it would be measuring that instead of what it meant to.
func withTempImageCache(t *testing.T, appName string) {
	t.Helper()

	dir := t.TempDir()
	switch runtime.GOOS {
	case "windows":
		t.Setenv("APPDATA", dir)
	case "darwin":
		t.Setenv("HOME", dir)
	default:
		t.Setenv("XDG_CONFIG_HOME", dir)
	}

	internal.SetAppName(appName)

	t.Cleanup(func() {
		if cache := internal.SwapImageCache(nil); cache != nil {
			_ = cache.Close()
		}
	})
}

// The regression this whole change exists for. Initialize used to open the new store before swapping out the old one,
// so a second call asked Badger for the directory lock the same process was already holding, failed, and returned
// before reaching the swap that would have released it. Every retry then failed for the life of the process.
//
// The GUI reaches this through its error boundary's Reload button, which remounts the frontend and calls Initialize
// again inside a Go process whose Badger handle survived.
func TestSetupImageCacheTwiceKeepsTheSameStore(t *testing.T) {
	withTempImageCache(t, "opai-setup-twice")

	setupImageCache()

	first := internal.ImageCache()
	if first == nil {
		t.Fatal("the first setupImageCache() installed no cache")
	}
	if first.Mode() != types.CacheModeDisk {
		t.Fatalf("the first setupImageCache() gave mode %q, want disk", first.Mode())
	}

	setupImageCache()

	second := internal.ImageCache()
	if second == nil {
		t.Fatal("the second setupImageCache() left no cache installed; it must not fail on its own lock")
	}
	if second != first {
		t.Error("the second setupImageCache() replaced the store; the one already open should be kept")
	}
	if second.Mode() != types.CacheModeDisk {
		t.Errorf("the second setupImageCache() degraded to mode %q; it should still be on disk", second.Mode())
	}
}

// The one case that must reopen: a different app name is a different config directory, so the store already open is
// not the one this call is asking for. No lock is contended either way, since the two directories are distinct.
func TestSetupImageCacheReopensWhenTheAppNameChanges(t *testing.T) {
	withTempImageCache(t, "opai-setup-rename-a")

	setupImageCache()

	first := internal.ImageCache()
	if first == nil {
		t.Fatal("setupImageCache() installed no cache")
	}

	internal.SetAppName("opai-setup-rename-b")
	setupImageCache()

	second := internal.ImageCache()
	if second == nil {
		t.Fatal("setupImageCache() installed no cache after the app name changed")
	}
	if second == first {
		t.Fatal("setupImageCache() kept the old store after the app name changed")
	}
	if second.Path() == first.Path() {
		t.Errorf("both stores resolved to %q; the app name should have moved the directory", second.Path())
	}
}

// A cache directory that genuinely cannot be opened must degrade rather than fail: losing the cache costs speed and
// nothing else.
//
// Making it unopenable takes a read-only directory now. Holding a second handle open used to do it, which is what
// this test did before, but that is precisely the case memo.NewDiskShared turned into sharing - so the old setup
// would now succeed and the fallback would never be exercised.
func TestSetupImageCacheFallsBackToMemory(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("directory permissions do not stop writes the same way on Windows")
	}
	if os.Geteuid() == 0 {
		t.Skip("running as root, which ignores the directory permissions this test relies on")
	}

	withTempImageCache(t, "opai-setup-fallback")

	// Created up front and then made read-only, so ConfigDir's MkdirAll still succeeds - it is a no-op on a directory
	// that exists - and Badger is the thing that fails, which is the path under test.
	cachePath, err := internal.ConfigDir("cache")
	if err != nil {
		t.Fatalf("ConfigDir() failed: %v", err)
	}
	if err = os.Chmod(cachePath, 0o500); err != nil {
		t.Fatalf("could not make the cache directory read-only: %v", err)
	}
	t.Cleanup(func() { _ = os.Chmod(cachePath, 0o700) })

	setupImageCache()

	cache := internal.ImageCache()
	if cache == nil {
		t.Fatal("setupImageCache() installed no cache; it should have fallen back to memory")
	}
	if cache.Mode() != types.CacheModeMemory {
		t.Errorf("Mode() = %q, want memory", cache.Mode())
	}
	if ImageCacheMode() != types.CacheModeMemory {
		t.Errorf("ImageCacheMode() = %q, want memory", ImageCacheMode())
	}
}

// cmd/cli and cmd/perf disable the cache before calling Initialize, and used to pay for the store anyway - taking a
// directory lock they never used, which a GUI running beside them then failed on.
//
// The proof is that the cache directory does not exist afterwards. Reopening it would prove nothing now: a shared
// open succeeds whether or not this process is already holding the store, which is the whole point of NewDiskShared.
func TestSetupImageCacheSkippedWhenDisabled(t *testing.T) {
	withTempImageCache(t, "opai-setup-disabled")

	internal.SetImageCacheEnabled(false)
	t.Cleanup(func() { internal.SetImageCacheEnabled(true) })

	setupImageCache()

	if cache := internal.ImageCache(); cache != nil {
		t.Fatalf("setupImageCache() installed a %q cache while the cache was disabled", cache.Mode())
	}
	if ImageCacheMode() != types.CacheModeNone {
		t.Errorf("ImageCacheMode() = %q, want none", ImageCacheMode())
	}

	// Built by hand rather than through ConfigDir, which would create the very directory being asserted absent.
	configDir, err := os.UserConfigDir()
	if err != nil {
		t.Fatalf("UserConfigDir() failed: %v", err)
	}

	cachePath := filepath.Join(configDir, internal.AppName(), "cache")
	if _, err = os.Stat(cachePath); !os.IsNotExist(err) {
		t.Errorf("%s exists, so a store was opened despite the cache being disabled", cachePath)
	}
}

// Concurrent setup must leave exactly one usable cache installed. Two Initialize calls can genuinely overlap - the
// GUI's error boundary reloads the frontend, which starts a second one while this process is still inside the first.
//
// Read what this does and does not prove. It is a smoke test: it pins that concurrent callers end up on the disk
// store and that the path is clean under -race. It does NOT reproduce the leak the mutex exists to prevent, and it
// passes without that mutex. The damaging interleaving needs a loser of the race to swap its memory fallback in
// *after* the winner installs the disk store, and the winner is the goroutine inside a ~40 ms Badger open while every
// loser fails instantly - so the winner almost always swaps last and the leaked handle is the loser's memory cache
// rather than the disk one. Forcing the other order would need a hook in setupImageCache itself, which is a worse
// trade than an honest comment: the mutex is two lines and makes the ordering question moot.
func TestSetupImageCacheIsSafeConcurrently(t *testing.T) {
	withTempImageCache(t, "opai-setup-concurrent")

	var wg sync.WaitGroup
	for range 8 {
		wg.Add(1)

		go func() {
			defer wg.Done()
			setupImageCache()
		}()
	}

	wg.Wait()

	cache := internal.ImageCache()
	if cache == nil {
		t.Fatal("no cache is installed after concurrent setup")
	}
	if cache.Mode() != types.CacheModeDisk {
		t.Errorf("Mode() = %q, want disk - a caller lost the race for the directory lock and degraded", cache.Mode())
	}
}
