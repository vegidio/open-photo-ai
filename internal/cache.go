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

type Cache struct {
	diskCache *memo.Memoizer
}

func NewCache(maxEntries int64) (*Cache, error) {
	// AppName, not a hardcoded name: Initialize promises the caller a config directory under the name it passed, and
	// the model cache already honours that. Hardcoding here would split an embedder's two caches across two directories.
	cachePath, err := fs.MkUserConfigDir(AppName, "cache")
	if err != nil {
		return nil, errors.Wrap(err, "failed to create cache directory")
	}

	const capacity = 1024 * 1024 * 1000 // 1 GB
	opts := memo.CacheOpts{MaxEntries: maxEntries, MaxCapacity: capacity}
	diskCache, err := memo.NewDiskOnly(cachePath, opts)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create disk cache")
	}

	return &Cache{
		diskCache: diskCache,
	}, nil
}

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

func (c *Cache) SetImage(ctx context.Context, img image.Image, hash string, operations ...types.Operation) error {
	data, err := imageToData(img)
	if err != nil {
		return err
	}

	key := cacheKey(hash, operations)
	ttl := time.Hour * 24

	return c.diskCache.Store.Set(ctx, key, data, ttl)
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
