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
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/memo"
	"github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/wailsapp/wails/v3/pkg/application"
	"golang.org/x/image/tiff"
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
//     equals the specified size.
//
// # Returns:
//   - []byte: The image data encoded as PNG bytes (lossless)
//   - int: The width of the image
//   - int: The height of the image
//   - error: An error if the image cannot be loaded, processed, or encoded
func (i *ImageService) GetImage(filePath string, size int) ([]byte, int, int, error) {
	inputData, err := opai.LoadImage(filePath)
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

// ProcessImage runs inference operations on an image and returns the processed result.
//
// # Parameters:
//   - filePath: The path to the image file to process.
//   - opIds: Variable number of operation IDs specifying the inference operations to apply to the image.
//     Each operation ID encodes the model name, parameters, and precision.
//
// # Returns:
//   - []byte: The processed image data encoded as JPEG bytes for presentation purposes.
//   - int: The width of the processed image.
//   - int: The height of the processed image.
//   - error: An error if the inference fails or the image cannot be processed.
func (i *ImageService) ProcessImage(filePath string, opIds ...string) ([]byte, int, int, error) {
	pngBytes, err := i.runInference(filePath, opIds)
	if err != nil {
		return nil, 0, 0, err
	}

	img, err := bytesToImage(pngBytes)
	if err != nil {
		return nil, 0, 0, err
	}

	jpgBytes, err := imageToBytes(img, types.FormatJpeg)
	if err != nil {
		return nil, 0, 0, err
	}

	// Return a version of the image as JPG for presentation purposes
	bounds := img.Bounds()
	return jpgBytes, bounds.Dx(), bounds.Dy(), nil
}

// ExportImage runs inference operations on an image and saves the result to disk.
//
// # Parameters:
//   - inputPath: The path to the image file to process.
//   - outputPath: The path to the output file to save the processed image.
//   - format: The image format to use when saving the processed imag.
//   - opIds: Variable number of operation IDs specifying the inference operations to apply to the image.
//     Each operation ID encodes the model name, parameters, and precision.
//
// # Returns:
//   - error: An error if the inference fails, the image cannot be processed, or the file cannot be saved.
func (i *ImageService) ExportImage(inputPath, outputPath string, format types.ImageFormat, opIds ...string) error {
	pngBytes, err := i.runInference(inputPath, opIds)
	if err != nil {
		return err
	}

	img, err := bytesToImage(pngBytes)
	if err != nil {
		return err
	}

	return opai.SaveImage(&types.ImageData{
		FilePath: outputPath,
		Pixels:   img,
	}, format, 100)
}

func (i *ImageService) Destroy() {
	if i.diskCache != nil {
		i.diskCache.Close()
	}
}

// region - Private methods

func (i *ImageService) runInference(filePath string, opIds []string) ([]byte, error) {
	// Cache the image as PNG to be reused later
	ctx := context.Background()
	key := getCacheKey(filePath, opIds)
	ttl := time.Hour * 24

	return memo.Do(i.diskCache, ctx, key, ttl, func(ctx context.Context) ([]byte, error) {
		inputImage, err := opai.LoadImage(filePath)
		if err != nil {
			return nil, err
		}

		operations := idsToOperations(opIds)
		outputData, err := opai.Process(inputImage, func(name string, progress float64) {
			i.app.Event.Emit("app:progress", name, progress)
		}, operations...)

		if err != nil {
			return nil, err
		}

		// Convert to PNG bytes since it's lossless
		return imageToBytes(outputData.Pixels, types.FormatPng)
	})
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
		case "santorini":
			precision := types.Precision(values[2])
			operations = append(operations, santorini.Op(precision))

		// Upscale
		case "tokyo":
			scale, _ := strconv.Atoi(values[2])
			precision := types.Precision(values[3])
			operations = append(operations, tokyo.Op(scale, precision))
		case "kyoto":
			mode := kyoto.Mode(values[2])
			scale, _ := strconv.Atoi(values[3])
			precision := types.Precision(values[4])
			operations = append(operations, kyoto.Op(mode, scale, precision))
		}
	}

	return operations
}

func getCacheKey(filePath string, opIds []string) string {
	key := lo.Reduce(opIds, func(agg string, item string, _ int) string {
		if len(agg) == 0 {
			return item
		}
		return agg + "|" + item
	}, "")

	return memo.KeyFrom(filePath, key)
}

func imageToBytes(img image.Image, format types.ImageFormat) ([]byte, error) {
	var buf bytes.Buffer

	switch format {
	case types.FormatJpeg:
		if err := jpeg.Encode(&buf, img, &jpeg.Options{Quality: 100}); err != nil {
			return nil, err
		}
	case types.FormatPng:
		encoder := &png.Encoder{CompressionLevel: png.DefaultCompression}
		if err := encoder.Encode(&buf, img); err != nil {
			return nil, err
		}
	case types.FormatTiff:
		if err := tiff.Encode(&buf, img, &tiff.Options{Compression: tiff.Deflate}); err != nil {
			return nil, err
		}
	default:
		return nil, fmt.Errorf("unsupported image format: %d", format)
	}

	return buf.Bytes(), nil
}

func bytesToImage(data []byte) (image.Image, error) {
	img, _, err := image.Decode(bytes.NewReader(data))
	return img, err
}

// endregion
