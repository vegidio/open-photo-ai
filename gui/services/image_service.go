package services

import (
	"bytes"
	"context"
	"image"
	"image/png"
	"time"

	"github.com/disintegration/imaging"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/memo"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale"
)

type ImageService struct {
	appName   string
	memCache  *memo.Memoizer
	diskCache *memo.Memoizer
}

func NewImageService(appName string) *ImageService {
	return &ImageService{appName: appName}
}

// GetImage loads an image from the specified file path and optionally resizes it.
// The method uses an in-memory cache to store processed images for faster later access.
//
// # Parameters:
//   - filePath: The path to the image file to load
//   - size: The target size for the longest dimension of the image. If size is 0, the image is returned at its original
//     dimensions. If size > 0, the image is resized proportionally so that its longest dimension (width or height)
//     equals the specified size, using Lanczos resampling for high quality.
//
// # Returns:
//   - []byte: The image data encoded as PNG bytes (lossless)
//   - error: An error if the image cannot be loaded, processed, or encoded
//
// The method initializes a memory cache on first use with a capacity of 100 MB and a maximum of 100 entries. Cached
// images have a TTL of 24 hours or until the app is closed.
func (i *ImageService) GetImage(filePath string, size int) ([]byte, error) {
	if i.memCache == nil {
		var err error
		opts := memo.CacheOpts{MaxEntries: 100, MaxCapacity: 1024 * 1024 * 100}
		i.memCache, err = memo.NewMemoryOnly(opts)
		if err != nil {
			return nil, err
		}
	}

	ctx := context.Background()
	key := memo.KeyFrom(filePath, size)
	ttl := time.Hour * 24

	return memo.Do(i.memCache, ctx, key, ttl, func(ctx context.Context) ([]byte, error) {
		inputData, err := opai.LoadInputData(filePath)
		if err != nil {
			return nil, err
		}

		if size > 0 {
			bounds := inputData.Pixels.Bounds()
			if bounds.Dx() >= bounds.Dy() {
				inputData.Pixels = imaging.Resize(inputData.Pixels, size, 0, imaging.Lanczos)
			} else {
				inputData.Pixels = imaging.Resize(inputData.Pixels, 0, size, imaging.Lanczos)
			}
		}

		return imageToBytes(inputData.Pixels)
	})
}

func (i *ImageService) ProcessImage(filePath string) ([]byte, error) {
	if i.diskCache == nil {
		cachePath, err := fs.MkUserConfigDir(i.appName, "cache", "images")
		if err != nil {
			return nil, err
		}

		opts := memo.CacheOpts{MaxEntries: 100, MaxCapacity: 1024 * 1024 * 500}
		i.diskCache, err = memo.NewDiskOnly(cachePath, opts)
		if err != nil {
			return nil, err
		}
	}

	operation := upscale.Op(4, "high")
	ctx := context.Background()
	key := memo.KeyFrom(filePath, operation.Id())
	ttl := time.Hour * 24

	return memo.Do(i.diskCache, ctx, key, ttl, func(ctx context.Context) ([]byte, error) {
		inputData, err := opai.LoadInputData(filePath)
		if err != nil {
			return nil, err
		}

		outputData, err := opai.Execute(inputData, operation)
		if err != nil {
			return nil, err
		}

		return imageToBytes(outputData.Pixels)
	})
}

// region - Private methods

func (i *ImageService) Destroy() {
	if i.memCache != nil {
		i.memCache.Close()
	}

	if i.diskCache != nil {
		i.diskCache.Close()
	}
}

// endregion

// region - Private functions

func imageToBytes(img image.Image) ([]byte, error) {
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		return nil, err
	}

	return buf.Bytes(), nil
}

// endregion
