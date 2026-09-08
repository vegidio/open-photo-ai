package internal

import (
	"bytes"
	"context"
	"image"
	"image/png"
	"strings"
	"sync/atomic"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/memo"
	"github.com/vegidio/open-photo-ai/types"
)

// imageCacheDisabled is named for the disabled state so that the zero value keeps the cache on: an embedder that never
// calls SetImageCacheEnabled must see the default behavior. Safe for concurrent use.
var imageCacheDisabled atomic.Bool

// SetImageCacheEnabled toggles the per-operation image cache. Safe for concurrent use.
func SetImageCacheEnabled(enabled bool) {
	imageCacheDisabled.Store(!enabled)
}

// ImageCacheEnabled reports whether Process should read from and write to the image cache.
func ImageCacheEnabled() bool {
	return !imageCacheDisabled.Load()
}

const (
	// cacheCapacityBytes bounds the store on disk. It is 1000 MiB rather than a round gigabyte of either kind - the
	// figure has no significance beyond "about a gigabyte", which is what the comment here used to claim inaccurately.
	cacheCapacityBytes = 1024 * 1024 * 1000

	// memoryCacheCapacityBytes bounds the in-memory fallback, and is two orders of magnitude below the disk ceiling
	// above because it is not the same quantity. For the memory store MaxCapacity is ristretto's MaxCost, a real RAM
	// ceiling it evicts to stay under; for Badger it only sizes the value-log files and caps nothing. It is also
	// claimed at the worst possible moment - the fallback is reached when the disk store would not open, which in
	// production is often a machine that has just run out of memory.
	memoryCacheCapacityBytes = 64 * 1024 * 1024

	// cacheEntryTTL is how long a processed image stays worth keeping. A day covers a working session, which is the
	// span over which someone re-runs the same enhancement on the same photo; past that the pixels are cheaper to
	// recompute than to keep.
	cacheEntryTTL = 24 * time.Hour
)

// ErrNotAdmitted reports a write the cache dropped instead of storing: the memory store's set buffer was full, or the
// store is closing. Nothing is lost but a future hit, so it is worth separating from a store that actually broke.
//
// Aliased rather than re-derived so that errors.Is works against what the store actually returns, and re-exported here
// rather than imported at each call site so the layers above keep treating the cache as a Cache rather than as a memo
// store. It used to be matched by message text, because memo kept the sentinel in a package no caller could import.
var ErrNotAdmitted = memo.ErrNotAdmitted

// Cache is the store of processed images, keyed by source pixels plus the operations applied to them. It is what makes
// re-running a chain the user has already seen cost nothing.
type Cache struct {
	store *memo.Memoizer

	// mode says which store is behind this cache. It exists to be reported rather than branched on: everything below
	// works the same either way, but a run that silently lost its disk cache is a run nobody can explain afterwards.
	mode types.CacheMode
}

// NewCache opens the store under the config directory, bounded by maxEntries and by cacheCapacityBytes.
func NewCache(maxEntries int64) (*Cache, error) {
	// AppName, not a hardcoded name: Initialize promises the caller a config directory under the name it passed, and
	// the model cache already honours that. Hardcoding here would split an embedder's two caches across two directories.
	cachePath, err := ConfigDir("cache")
	if err != nil {
		return nil, err
	}

	// Shared rather than exclusive: Badger's directory lock is per directory and is not reentrant, so opening a path
	// this process already holds used to fail with an error indistinguishable from a second copy of the app holding
	// it. NewDiskShared hands back the store already open instead, which leaves that error meaning only what it says.
	opts := memo.CacheOpts{MaxEntries: maxEntries, MaxCapacity: cacheCapacityBytes}
	diskCache, err := memo.NewDiskShared(cachePath, opts)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create disk cache")
	}

	return &Cache{
		store: diskCache,
		mode:  types.CacheModeDisk,
	}, nil
}

// NewMemoryCache opens a store with no directory behind it, for the run that could not have the disk one.
//
// What is left after NewDiskShared is a directory another *process* holds - a second copy of the app - or one that
// cannot be written to at all. Badger's lock is not waitable, so neither has a retry that can succeed. Caching in RAM
// keeps a repeated enhancement fast in that run anyway; the cost is that the results die with the process.
func NewMemoryCache(maxEntries int64) (*Cache, error) {
	return newMemoryCache(maxEntries, memoryCacheCapacityBytes)
}

// newMemoryCache takes the ceiling as a parameter so a test can exercise the eviction and admission behaviour against
// a small image rather than having to build one that exceeds the real 64 MiB budget.
func newMemoryCache(maxEntries, capacity int64) (*Cache, error) {
	opts := memo.CacheOpts{MaxEntries: maxEntries, MaxCapacity: capacity}

	memCache, err := memo.NewMemoryOnly(opts)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create memory cache")
	}

	return &Cache{
		store: memCache,
		mode:  types.CacheModeMemory,
	}, nil
}

// Mode reports which store is backing this cache.
func (c *Cache) Mode() types.CacheMode {
	return c.mode
}

// Path reports the directory a disk cache was opened under. It is empty for a memory cache.
func (c *Cache) Path() string {
	return c.store.Path()
}

// GetImage returns the stored result of applying operations to the image identified by hash, or an error when there is
// none. Every failure is reported as a miss - see the body for why a store error is not propagated.
func (c *Cache) GetImage(ctx context.Context, hash string, operations ...types.Operation) (image.Image, error) {
	key := cacheKey(hash, operations)

	data, found, err := c.store.Store.Get(ctx, key)
	if err != nil {
		// A real store error (e.g. disk failure) is reported as a miss so the caller re-runs inference, but it's logged
		// so a failing cache doesn't degrade silently.
		Log().Warn("cache lookup failed", "key", key, "err", err)
		return nil, errors.Errorf("cache miss for key: %s", key)
	}

	if !found {
		return nil, errors.Errorf("cache miss for key: %s", key)
	}

	img, err := dataToImage(data)
	if err != nil {
		// A stored entry that no longer decodes - a truncated write, a half-flushed shutdown - is reported as a miss
		// like any other, so the caller re-runs the inference and SetImage overwrites this key with a good value. It
		// self-heals, but silently: the only visible symptom is one enhancement that was inexplicably slow. Saying so
		// is what makes that one slow run explicable in a log a user attached to a bug report.
		Log().Warn("a cached image is corrupt; re-running the operation to replace it", "key", key, "err", err)
		return nil, errors.Wrap(err, "failed to decode image")
	}

	return img, nil
}

// SetImage stores img as the result of applying operations to the image identified by hash.
func (c *Cache) SetImage(ctx context.Context, img image.Image, hash string, operations ...types.Operation) error {
	data, err := imageToData(img)
	if err != nil {
		return err
	}

	key := cacheKey(hash, operations)

	// A dropped write comes back as ErrNotAdmitted and a broken store as anything else, which is the distinction the
	// caller acts on; both are passed through unchanged.
	return c.store.Store.Set(ctx, key, data, cacheEntryTTL)
}

// ImageHashAfter is the identity of the pixels produced by applying operations to the image identified by hash.
//
// It is the cache key, exported under a name that says what it means to a caller: ImageData.Hash identifies Pixels, so
// anything that hands back transformed pixels has to hand back a hash that moved with them. Sharing one derivation
// with the cache is the point - a result fed back into Process then looks up exactly the slot its pixels were stored
// under.
func ImageHashAfter(hash string, operations []types.Operation) string {
	// Applying nothing leaves the pixels alone, so the identity is unchanged. Owned here rather than at each caller:
	// it is a property of the derivation, not something every caller has to remember about it.
	if len(operations) == 0 {
		return hash
	}

	return cacheKey(hash, operations)
}

func cacheKey(hash string, operations []types.Operation) string {
	ops := lo.Map(operations, func(op types.Operation, _ int) string {
		id := op.Id()

		// Per-run inputs (e.g. the selected faces) are not encoded in Id() but change the output, so fold them in.
		if ck, ok := op.(types.CacheKeyer); ok {
			if extra := ck.CacheKey(); extra != "" {
				id += "#" + extra
			}
		}

		return id
	})
	return memo.KeyFrom(hash, strings.Join(ops, "|"))
}

// Close flushes and releases the underlying store. The caller must not use the Cache afterwards.
func (c *Cache) Close() error {
	return c.store.Close()
}

// region - Private functions

func imageToData(img image.Image) ([]byte, error) {
	var buf bytes.Buffer

	encoder := &png.Encoder{CompressionLevel: png.BestSpeed}
	if err := encoder.Encode(&buf, img); err != nil {
		return nil, errors.Wrap(err, "failed to encode image")
	}

	return buf.Bytes(), nil
}

func dataToImage(data []byte) (image.Image, error) {
	img, _, err := image.Decode(bytes.NewReader(data))
	return img, err
}

// endregion
