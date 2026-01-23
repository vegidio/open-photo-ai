package internal

import (
	"bytes"
	"context"
	"fmt"
	"image"
	"image/png"
	"strings"
	"time"

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
		return nil, err
	}

	const capacity = 1024 * 1024 * 1000 // 1 GB
	opts := memo.CacheOpts{MaxEntries: maxEntries, MaxCapacity: capacity}
	diskCache, err := memo.NewDiskOnly(cachePath, opts)
	if err != nil {
		return nil, err
	}

	return &Cache{
		diskCache: diskCache,
	}, nil
}

func (c *Cache) GetImage(hash string, operations ...types.Operation) (image.Image, error) {
	ops := lo.Map(operations, func(op types.Operation, _ int) string {
		return op.Id()
	})
	key := memo.KeyFrom(hash, strings.Join(ops, "|"))

	data, found, err := c.diskCache.Store.Get(context.Background(), key)
	if err != nil || !found {
		return nil, fmt.Errorf("cache miss for key: %s, %w", key, err)
	}

	img, err := dataToImage(data)
	if err != nil {
		return nil, err
	}

	return img, nil
}

func (c *Cache) SetImage(img image.Image, hash string, operations ...types.Operation) error {
	data, err := imageToData(img)
	if err != nil {
		return err
	}

	ops := lo.Map(operations, func(op types.Operation, _ int) string {
		return op.Id()
	})
	key := memo.KeyFrom(hash, strings.Join(ops, "|"))

	ttl := time.Hour * 24

	return c.diskCache.Store.Set(context.Background(), key, data, ttl)
}

func (c *Cache) Close() error {
	return c.diskCache.Close()
}

// region - Private functions

func imageToData(img image.Image) ([]byte, error) {
	var buf bytes.Buffer

	encoder := &png.Encoder{CompressionLevel: png.BestSpeed}
	if err := encoder.Encode(&buf, img); err != nil {
		return nil, err
	}

	return buf.Bytes(), nil
}

func dataToImage(data []byte) (image.Image, error) {
	img, _, err := image.Decode(bytes.NewReader(data))
	return img, err
}

// endregion
