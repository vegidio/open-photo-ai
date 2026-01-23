package services

import (
	"context"
	"fmt"
	guitypes "gui/types"
	guiutils "gui/utils"
	"path/filepath"
	"strings"

	"github.com/disintegration/imaging"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type ImageService struct {
	app *application.App
	tel *o11y.Telemetry
}

func NewImageService(app *application.App, tel *o11y.Telemetry) *ImageService {
	return &ImageService{
		app: app,
		tel: tel,
	}
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
func (s *ImageService) GetImage(filePath string, size int) ([]byte, int, int, error) {
	inputData, err := utils.LoadImage(filePath)
	if err != nil {
		s.tel.LogError("Error loading image", nil, err)
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

	data, err := utils.EncodeImage(inputData.Pixels, types.FormatJpeg, 90)
	if err != nil {
		s.tel.LogError("Error encoding image", nil, err)
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
func (s *ImageService) ProcessImage(ctx context.Context, filePath string, opIds ...string) ([]byte, int, int, error) {
	if err := ctx.Err(); err != nil {
		return nil, 0, 0, err
	}

	outputData, err := s.runInference(ctx, filePath, opIds)
	if err != nil {
		s.tel.LogError("Error running inference", map[string]any{
			"operations": strings.Join(opIds, ", "),
		}, err)

		return nil, 0, 0, err
	}

	if err = ctx.Err(); err != nil {
		return nil, 0, 0, err
	}

	data, err := utils.EncodeImage(outputData.Pixels, types.FormatJpeg, 90)
	if err != nil {
		s.tel.LogError("Error encoding image", nil, err)
		return nil, 0, 0, err
	}

	if err = ctx.Err(); err != nil {
		return nil, 0, 0, err
	}

	// Return a version of the image as JPG for presentation purposes
	bounds := outputData.Pixels.Bounds()
	return data, bounds.Dx(), bounds.Dy(), nil
}

// SuggestEnhancements analyzes an image and returns suggestions for enhancement operations.
//
// # Parameters:
//   - filePath: The path to the image file to analyze
//
// # Returns:
//   - []string: A slice of operation IDs representing suggested enhancements (e.g., "up_athens_4x_fp32").
//   - error: An error if the image cannot be loaded or the AI analysis fails.
func (s *ImageService) SuggestEnhancements(filePath string) ([]string, error) {
	inputImage, err := utils.LoadImage(filePath)
	if err != nil {
		s.tel.LogError("Error loading image", nil, err)
		return nil, err
	}

	operations, err := opai.SuggestEnhancements(inputImage)
	if err != nil {
		s.tel.LogError("Error suggesting enhancements", nil, err)
		return nil, err
	}

	return lo.Map(operations, func(op types.Operation, _ int) string {
		return op.Id()
	}), nil
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
func (s *ImageService) ExportImage(
	ctx context.Context,
	file guitypes.File,
	outputPath string,
	overwrite bool,
	format types.ImageFormat,
	opIds ...string,
) error {
	if err := ctx.Err(); err != nil {
		return err
	}

	eventName := fmt.Sprintf("app:export:%s", file.Hash)
	s.app.Event.Emit(eventName, "RUNNING", 0.1)

	outputData, err := s.runInference(ctx, file.Path, opIds)
	if err != nil {
		s.tel.LogError("Error running inference", map[string]any{
			"operations": strings.Join(opIds, ", "),
		}, err)

		return err
	}

	s.app.Event.Emit(eventName, "RUNNING", 0.9)
	if err = ctx.Err(); err != nil {
		return err
	}

	size, err := utils.SaveImage(&types.ImageData{
		FilePath: getOutputPath(outputPath, overwrite),
		Pixels:   outputData.Pixels,
	}, format, 100)

	if err != nil {
		s.tel.LogError("Error saving image", nil, err)
		return err
	}

	s.app.Event.Emit(eventName, "COMPLETED", size)
	return nil
}

// region - Private methods

func (s *ImageService) runInference(ctx context.Context, filePath string, opIds []string) (*types.ImageData, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	inputImage, err := utils.LoadImage(filePath)
	if err != nil {
		return nil, err
	}

	operations := guiutils.IdsToOperations(opIds)
	outputData, err := opai.Process(ctx, inputImage, nil, func(name string, progress float64) {
		s.app.Event.Emit("app:progress", name, progress)
	}, operations...)

	if err != nil {
		return nil, err
	}

	return outputData, nil
}

func (s *ImageService) destroy() {
	// Nothing to do here
}

// endregion

// region - Private functions

func getOutputPath(filePath string, overwrite bool) string {
	if overwrite {
		return filePath
	}

	ext := filepath.Ext(filePath)
	basePath := filePath[:len(filePath)-len(ext)]
	outputPath := basePath + ext
	count := 1

	for {
		if exists := fs.FileExists(outputPath); !exists {
			return outputPath
		}

		outputPath = fmt.Sprintf("%s_%d%s", basePath, count, ext)
		count++
	}
}

// endregion
