package services

import (
	"bytes"
	"context"
	"image"
	"image/jpeg"
	"image/png"
	"strconv"
	"strings"
	"time"

	"github.com/disintegration/imaging"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/memo"
	"github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type ImageService struct {
	diskCache *memo.Memoizer
	app       *application.App
}

func NewImageService(app *application.App) (*ImageService, error) {
	cachePath, err := fs.MkUserConfigDir("open-photo-ai", "cache", "images")
	if err != nil {
		return nil, err
	}

	opts := memo.CacheOpts{MaxEntries: 100, MaxCapacity: 1024 * 1024 * 500}
	diskCache, err := memo.NewDiskOnly(cachePath, opts)
	if err != nil {
		return nil, err
	}

	return &ImageService{diskCache: diskCache, app: app}, nil
}

// GetImage loads an image from the specified file path and optionally resizes it.
//
// # Parameters:
//   - filePath: The path to the image file to load
//   - size: The target size for the longest dimension of the image. If size is 0, the image is returned at its original
//     dimensions. If size > 0, the image is resized proportionally so that its longest dimension (width or height)
//     equals the specified size, using Lanczos resampling for high quality.
//
// # Returns:
//   - []byte: The image data encoded as PNG bytes (lossless)
//   - int: The width of the image
//   - int: The height of the image
//   - error: An error if the image cannot be loaded, processed, or encoded
func (i *ImageService) GetImage(filePath string, size int) ([]byte, int, int, error) {
	inputData, err := opai.LoadInputImage(filePath)
	if err != nil {
		return nil, 0, 0, err
	}

	if size > 0 {
		bounds := inputData.Pixels.Bounds()
		if bounds.Dx() >= bounds.Dy() {
			inputData.Pixels = imaging.Resize(inputData.Pixels, size, 0, imaging.Lanczos)
		} else {
			inputData.Pixels = imaging.Resize(inputData.Pixels, 0, size, imaging.Lanczos)
		}
	}

	data, err := imageToBytes(inputData.Pixels, types.FormatJpeg)
	if err != nil {
		return nil, 0, 0, err
	}

	bounds := inputData.Pixels.Bounds()
	return data, bounds.Dx(), bounds.Dy(), nil
}

func (i *ImageService) ProcessImage(filePath string, opIds ...string) ([]byte, int, int, error) {
	inputImage, err := opai.LoadInputImage(filePath)
	if err != nil {
		return nil, 0, 0, err
	}

	operations := idsToOperations(opIds)
	outputData, err := opai.Process(inputImage, func(name string, progress float64) {
		i.app.Event.Emit("app:progress", name, progress)
	}, operations...)

	if err != nil {
		return nil, 0, 0, err
	}

	pngBytes, err := imageToBytes(outputData.Pixels, types.FormatPng)
	if err != nil {
		return nil, 0, 0, err
	}

	// Cache the image as lossless PNG to be reused later
	ctx := context.Background()
	key := memo.KeyFrom(filePath, operations[0].Id())
	ttl := time.Hour * 24

	err = i.diskCache.Store.Set(ctx, key, pngBytes, ttl)
	if err != nil {
		return nil, 0, 0, err
	}

	data, err := imageToBytes(outputData.Pixels, types.FormatJpeg)
	if err != nil {
		return nil, 0, 0, err
	}

	// Return a version of the image as JPG for presentation purposes
	bounds := outputData.Pixels.Bounds()
	return data, bounds.Dx(), bounds.Dy(), nil
}

// region - Private methods

func (i *ImageService) Destroy() {
	if i.diskCache != nil {
		i.diskCache.Close()
	}
}

// endregion

// region - Private functions

func idsToOperations(opIds []string) []types.Operation {
	operations := make([]types.Operation, 0)

	for _, opId := range opIds {
		values := strings.Split(opId, "_")
		name := values[1]

		switch name {
		// Face Recovery
		case "athens":
			precision := types.Precision(values[2])
			operations = append(operations, athens.Op(precision))

		// Upscale
		case "kyoto":
			mode := kyoto.Mode(values[2])
			scale, _ := strconv.Atoi(values[3])
			precision := types.Precision(values[4])
			operations = append(operations, kyoto.Op(mode, scale, precision))
		}
	}

	return operations
}

func imageToBytes(img image.Image, format types.ImageFormat) ([]byte, error) {
	var buf bytes.Buffer

	switch format {
	case types.FormatJpeg:
		if err := jpeg.Encode(&buf, img, &jpeg.Options{Quality: 100}); err != nil {
			return nil, err
		}
	case types.FormatPng:
		encoder := &png.Encoder{CompressionLevel: png.NoCompression}
		if err := encoder.Encode(&buf, img); err != nil {
			return nil, err
		}
	default:
		panic("unhandled default case")
	}

	return buf.Bytes(), nil
}

// endregion
