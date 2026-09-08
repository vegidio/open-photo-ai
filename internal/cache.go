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
	"github.com/vegidio/go-sak/fs"
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

	// cacheEntryTTL is how long a processed image stays worth keeping. A day covers a working session, which is the
	// span over which someone re-runs the same enhancement on the same photo; past that the pixels are cheaper to
	// recompute than to keep.
	cacheEntryTTL = 24 * time.Hour
)

// Cache is the on-disk store of processed images, keyed by source pixels plus the operations applied to them. It is
// what makes re-running a chain the user has already seen cost nothing.
type Cache struct {
	diskCache *memo.Memoizer
}

// NewCache opens the store under the config directory, bounded by maxEntries and by cacheCapacityBytes.
func NewCache(maxEntries int64) (*Cache, error) {
	// AppName, not a hardcoded name: Initialize promises the caller a config directory under the name it passed, and
	// the model cache already honours that. Hardcoding here would split an embedder's two caches across two directories.
	cachePath, err := fs.MkUserConfigDir(AppName(), "cache")
	if err != nil {
		return nil, errors.Wrap(err, "failed to create cache directory")
	}

	opts := memo.CacheOpts{MaxEntries: maxEntries, MaxCapacity: cacheCapacityBytes}
	diskCache, err := memo.NewDiskOnly(cachePath, opts)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create disk cache")
	}

	return &Cache{
		diskCache: diskCache,
	}, nil
}

// GetImage returns the stored result of applying operations to the image identified by hash, or an error when there is
// none. Every failure is reported as a miss - see the body for why a store error is not propagated.
func (c *Cache) GetImage(ctx context.Context, hash string, operations ...types.Operation) (image.Image, error) {
	key := cacheKey(hash, operations)

	data, found, err := c.diskCache.Store.Get(ctx, key)
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

	return c.diskCache.Store.Set(ctx, key, data, cacheEntryTTL)
}

// ImageHashAfter is the identity of the pixels produced by applying operations to the image identified by hash.
//
// It is the cache key, exported under a name that says what it means to a caller: ImageData.Hash identifies Pixels, so
// anything that hands back transformed pixels has to hand back a hash that moved with them. Sharing one derivation
// with the cache is the point - a result fed back into Process then looks up exactly the slot its pixels were stored
// under.
func ImageHashAfter(hash string, operations []types.Operation) string {
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
	return c.diskCache.Close()
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
