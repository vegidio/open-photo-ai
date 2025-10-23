package services

import (
	"bytes"
	"context"
	"fmt"
	"image"
	"image/jpeg"
	"image/png"
	"strconv"
	"strings"
	"time"

	"github.com/disintegration/imaging"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/memo"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

type ImageService struct {
	diskCache *memo.Memoizer
}

func NewImageService(appName string) (*ImageService, error) {
	cachePath, err := fs.MkUserConfigDir(appName, "cache", "images")
	if err != nil {
		return nil, err
	}

	opts := memo.CacheOpts{MaxEntries: 100, MaxCapacity: 1024 * 1024 * 500}
	diskCache, err := memo.NewDiskOnly(cachePath, opts)
	if err != nil {
		return nil, err
	}

	return &ImageService{diskCache}, nil
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
//   - error: An error if the image cannot be loaded, processed, or encoded
func (i *ImageService) GetImage(filePath string, size int) ([]byte, error) {
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

	return imageToBytes(inputData.Pixels, types.FormatJpeg)
}

func (i *ImageService) ProcessImage(filePath string, opIds ...string) ([]byte, error) {
	fmt.Println("start", filePath)

	inputData, err := opai.LoadInputData(filePath)
	if err != nil {
		return nil, err
	}

	operations := idsToOperations(opIds)
	outputData, err := opai.Execute(inputData, operations...)
	if err != nil {
		return nil, err
	}

	pngBytes, err := imageToBytes(outputData.Pixels, types.FormatPng)
	if err != nil {
		return nil, err
	}

	// Cache the image as lossless PNG to be reused later
	ctx := context.Background()
	key := memo.KeyFrom(filePath, operations[0].Id())
	ttl := time.Hour * 24

	err = i.diskCache.Store.Set(ctx, key, pngBytes, ttl)
	if err != nil {
		return nil, err
	}

	fmt.Println("end")

	// Return a version of the image as JPG for presentation purposes
	return imageToBytes(outputData.Pixels, types.FormatJpeg)
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
		name := values[0]

		switch name {
		case "upscale":
			scale, _ := strconv.Atoi(values[1])
			mode := upscale.Mode(values[2])

			fmt.Println(name, scale, mode)
			operations = append(operations, upscale.Op(scale, mode))
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
		if err := png.Encode(&buf, img); err != nil {
			return nil, err
		}
	default:
		panic("unhandled default case")
	}

	return buf.Bytes(), nil
}

// endregion
