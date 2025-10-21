package services

import (
	"bytes"
	"context"
	"image"
	"image/png"
	"time"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/memo"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale"
)

type ImageService struct {
	appName string
	memo    *memo.Memoizer
}

func NewImageService(appName string) *ImageService {
	return &ImageService{appName: appName}
}

func (i *ImageService) GetImage(filePath string) ([]byte, error) {
	inputData, err := opai.LoadInputData(filePath)
	if err != nil {
		return nil, err
	}

	return imageToBytes(inputData.Pixels)
}

func (i *ImageService) ProcessImage(filePath string) ([]byte, error) {
	if i.memo == nil {
		cachePath, err := fs.MkUserConfigDir(i.appName, "cache", "images")
		if err != nil {
			return nil, err
		}

		opts := memo.CacheOpts{MaxEntries: 100, MaxCapacity: 1024 * 1024 * 500}
		i.memo, err = memo.NewDiskOnly(cachePath, opts)
		if err != nil {
			return nil, err
		}
	}

	operation := upscale.Op(4, "high")
	ctx := context.Background()
	key := memo.KeyFrom(filePath, operation.Id())
	ttl := time.Hour * 24

	return memo.Do(i.memo, ctx, key, ttl, func(ctx context.Context) ([]byte, error) {
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
	if i.memo != nil {
		i.memo.Close()
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
