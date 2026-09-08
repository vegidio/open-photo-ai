package internal

import (
	"context"
	"image"
	"image/color"
	"math/rand"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/types"
)

// withTempConfigDir points ConfigDir at a directory of this test's own. NewCache resolves its path through
// os.UserConfigDir, and the real one may already hold a store belonging to the developer's installed copy of the app -
// which would make these tests fail on the very lock they are about to assert things about.
func withTempConfigDir(t *testing.T) {
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

	SetAppName("opai-cache-test")
}

// noiseImage is deliberately incompressible, so its PNG encoding is reliably larger than the small ceilings the
// admission test sets. A flat image would compress to a few hundred bytes and be admitted.
func noiseImage(size int) image.Image {
	img := image.NewRGBA(image.Rect(0, 0, size, size))
	rng := rand.New(rand.NewSource(1))

	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			img.Set(x, y, color.RGBA{
				R: uint8(rng.Intn(256)), G: uint8(rng.Intn(256)), B: uint8(rng.Intn(256)), A: 255,
			})
		}
	}

	return img
}

func TestNewCacheReportsItsDirectory(t *testing.T) {
	withTempConfigDir(t)

	cache, err := NewCache(16)
	if err != nil {
		t.Fatalf("NewCache() failed: %v", err)
	}
	t.Cleanup(func() { _ = cache.Close() })

	want, err := ConfigDir("cache")
	if err != nil {
		t.Fatalf("ConfigDir() failed: %v", err)
	}

	if cache.Path() != want {
		t.Errorf("Path() = %q, want %q", cache.Path(), want)
	}
	if cache.Mode() != types.CacheModeDisk {
		t.Errorf("Mode() = %q, want disk", cache.Mode())
	}
	if filepath.Base(cache.Path()) != "cache" {
		t.Errorf("Path() = %q, which does not end in the cache directory", cache.Path())
	}
}

// The invariant this fix now rests on, and it is the inverse of what it used to be. Badger's directory lock is per
// directory and not reentrant, so opening the same path twice in one process used to fail with an error that read
// exactly like a second copy of the app holding it - which is what made a repeat Initialize unrecoverable. NewCache
// goes through memo.NewDiskShared now, so the second open is another handle onto the same store.
func TestASecondCacheOnTheSameDirectoryShares(t *testing.T) {
	withTempConfigDir(t)

	first, err := NewCache(16)
	if err != nil {
		t.Fatalf("the first NewCache() failed: %v", err)
	}
	t.Cleanup(func() { _ = first.Close() })

	second, err := NewCache(16)
	if err != nil {
		t.Fatalf("the second NewCache() failed; opening a held path must share, not deadlock: %v", err)
	}
	t.Cleanup(func() { _ = second.Close() })

	if first.Path() != second.Path() {
		t.Fatalf("the two caches report different directories: %q and %q", first.Path(), second.Path())
	}

	// Sharing a directory is not the same as sharing a store, and only the second is any use here.
	ctx := context.Background()
	want := noiseImage(8)

	if err = first.SetImage(ctx, want, "shared-hash"); err != nil {
		t.Fatalf("SetImage() through the first handle failed: %v", err)
	}

	got, err := second.GetImage(ctx, "shared-hash")
	if err != nil {
		t.Fatalf("GetImage() through the second handle failed: %v", err)
	}
	if got.Bounds() != want.Bounds() {
		t.Errorf("GetImage() bounds = %v, want %v", got.Bounds(), want.Bounds())
	}
}

func TestMemoryCacheRoundTrips(t *testing.T) {
	withTempConfigDir(t)

	cache, err := NewMemoryCache(64)
	if err != nil {
		t.Fatalf("NewMemoryCache() failed: %v", err)
	}
	t.Cleanup(func() { _ = cache.Close() })

	if cache.Mode() != types.CacheModeMemory {
		t.Errorf("Mode() = %q, want memory", cache.Mode())
	}
	if cache.Path() != "" {
		t.Errorf("Path() = %q, want empty for a memory cache", cache.Path())
	}

	ctx := context.Background()
	want := noiseImage(8)

	if err = cache.SetImage(ctx, want, "hash-1"); err != nil {
		t.Fatalf("SetImage() failed: %v", err)
	}

	got, err := cache.GetImage(ctx, "hash-1")
	if err != nil {
		t.Fatalf("GetImage() failed: %v", err)
	}
	if got.Bounds() != want.Bounds() {
		t.Errorf("GetImage() bounds = %v, want %v", got.Bounds(), want.Bounds())
	}
}

// A dropped write must stay distinguishable from a broken store, since the caller warns about one and not the other.
// This used to be a text match against a message go-sak kept in a package nothing could import; memo exports the
// sentinel now, so the test is a real errors.Is and no longer breaks silently on a reword.
//
// A closed store is the only way to reach it deterministically: the other path is a full set buffer, which needs
// write contention to provoke and would make this test flaky. See TestMemoryCacheDropsOversized for why "too large"
// is deliberately not one of the ways to get here.
func TestSetImageReportsNotAdmitted(t *testing.T) {
	withTempConfigDir(t)

	cache, err := NewMemoryCache(64)
	if err != nil {
		t.Fatalf("NewMemoryCache() failed: %v", err)
	}
	if err = cache.Close(); err != nil {
		t.Fatalf("Close() failed: %v", err)
	}

	err = cache.SetImage(context.Background(), noiseImage(8), "hash-closed")
	if !errors.Is(err, ErrNotAdmitted) {
		t.Fatalf("SetImage() error = %v, want ErrNotAdmitted", err)
	}
}

// A value too large for the budget is dropped by ristretto's admission policy asynchronously, so the write still
// reports success and the entry is simply not there afterwards. Worth pinning: it is the reason the memory fallback
// can be given a small ceiling without every oversized result turning into an error the caller has to handle.
func TestMemoryCacheDropsOversized(t *testing.T) {
	withTempConfigDir(t)

	// A 1 KiB ceiling against a 64x64 noise PNG, which is far larger than that.
	cache, err := newMemoryCache(128, 1024)
	if err != nil {
		t.Fatalf("newMemoryCache() failed: %v", err)
	}
	t.Cleanup(func() { _ = cache.Close() })

	ctx := context.Background()
	if err = cache.SetImage(ctx, noiseImage(64), "hash-oversized"); err != nil {
		t.Fatalf("SetImage() error = %v, want nil - an oversized value is dropped, not refused", err)
	}

	if _, err = cache.GetImage(ctx, "hash-oversized"); err == nil {
		t.Error("GetImage() found an oversized entry; it should have been dropped by the admission policy")
	}
}
