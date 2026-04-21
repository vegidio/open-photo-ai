package internal

import (
	"bytes"
	"context"
	"image"
	"image/png"
	"strings"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/memo"
	"github.com/vegidio/open-photo-ai/types"
)

type Cache struct {
	diskCache *memo.Memoizer
}

func NewCache(maxEntries int64) (*Cache, error) {
	cachePath, err := fs.MkUserConfigDir("open-photo-ai", "cache")
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
	if !found || err != nil {
		return nil, errors.Errorf("cache miss for key: %s", key)
	}

	img, err := dataToImage(data)
	if err != nil {
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
		return op.Id()
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
