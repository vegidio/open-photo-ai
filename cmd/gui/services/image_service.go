package services

import (
	"context"
	"fmt"
	guitypes "gui/types"
	guiutils "gui/utils"
	"image"
	"log/slog"
	"path/filepath"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

const (
	previewJpegQuality  = 90
	exportMaxQuality    = 100
	progressInferStart  = 0.1
	progressInferEnd    = 0.9
	maxOutputDedupTries = 999
)

type ImageService struct {
	app  *application.App
	otel *o11y.Telemetry
}

func NewImageService(app *application.App, otel *o11y.Telemetry) *ImageService {
	return &ImageService{
		app:  app,
		otel: otel,
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
		s.otel.LogError("Error loading image", nil, err)
		slog.Error("error loading image", "file_path", filePath, "err", err)
		return nil, 0, 0, errors.Wrap(err, "failed to load image")
	}

	if size > 0 {
		bounds := inputData.Pixels.Bounds()
		if bounds.Dx() >= bounds.Dy() {
			inputData.Pixels = imaging.Resize(inputData.Pixels, size, 0, imaging.Lanczos)
		} else {
			inputData.Pixels = imaging.Resize(inputData.Pixels, 0, size, imaging.Lanczos)
		}
	}

	data, err := utils.EncodeImage(inputData.Pixels, types.FormatJpeg, previewJpegQuality)
	if err != nil {
		s.otel.LogError("Error encoding image", nil, err)
		slog.Error("error encoding image", "file_path", filePath, "err", err)
		return nil, 0, 0, errors.Wrap(err, "failed to encode image")
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
func (s *ImageService) ProcessImage(
	ctx context.Context,
	filePath string,
	ep types.ExecutionProvider,
	params guitypes.InferenceParams,
	opIds ...string,
) ([]byte, int, int, error) {
	if err := ctx.Err(); err != nil {
		return nil, 0, 0, errors.Wrap(err, "context cancelled")
	}

	ops := strings.Join(opIds, ", ")
	slog.Info("processing image", "file_path", filePath, "ep", ep, "operations", ops)

	outputData, err := s.runInference(ctx, filePath, ep, params, opIds)
	if err != nil {
		// Cancellation is expected (user navigated away / cancelled) — log it as info, not an error.
		if errors.Is(err, context.Canceled) {
			slog.Info("image processing cancelled", "file_path", filePath)
		} else {
			s.otel.LogError("Error running inference", map[string]any{
				"operations": ops,
			}, err)
			slog.Error("error running inference", "file_path", filePath,
				"operations", ops, "err", err)
		}

		return nil, 0, 0, errors.Wrap(err, "failed to run inference")
	}

	if err = ctx.Err(); err != nil {
		return nil, 0, 0, errors.Wrap(err, "context cancelled")
	}

	data, err := utils.EncodeImage(outputData.Pixels, types.FormatJpeg, previewJpegQuality)
	if err != nil {
		s.otel.LogError("Error encoding image", nil, err)
		slog.Error("error encoding processed image", "file_path", filePath, "err", err)
		return nil, 0, 0, errors.Wrap(err, "failed to encode image")
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
//   - []types.ModelType: A list of suggested enhancement types to apply to the image.
//   - error: An error if the image cannot be loaded.
func (s *ImageService) SuggestEnhancements(ctx context.Context, filePath string) ([]types.ModelType, error) {
	inputImage, err := utils.LoadImage(filePath)
	if err != nil {
		s.otel.LogError("Error loading image", nil, err)
		slog.Error("error loading image", "file_path", filePath, "err", err)
		return nil, errors.Wrap(err, "failed to load image")
	}

	suggestions := opai.SuggestEnhancements(ctx, inputImage)
	slog.Info("enhancements suggested", "file_path", filePath, "count", len(suggestions), "types", suggestions)
	return suggestions, nil
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
	ep types.ExecutionProvider,
	overwrite bool,
	format types.ImageFormat,
	params guitypes.InferenceParams,
	opIds ...string,
) error {
	if err := ctx.Err(); err != nil {
		return errors.Wrap(err, "context cancelled")
	}

	ops := strings.Join(opIds, ", ")
	slog.Info("exporting image", "input", file.Path, "output", outputPath,
		"format", format, "operations", ops)

	s.app.Event.Emit(EventAppExport, ExportUpdate{Hash: file.Hash, State: "RUNNING", Value: progressInferStart})

	outputData, err := s.runInference(ctx, file.Path, ep, params, opIds)
	if err != nil {
		// Cancellation is expected (user cancelled the export) — log it as info, not an error.
		if errors.Is(err, context.Canceled) {
			slog.Info("export cancelled", "hash", file.Hash, "input", file.Path)
		} else {
			s.otel.LogError("Error running inference", map[string]any{
				"operations": ops,
			}, err)
			slog.Error("error running inference", "input", file.Path,
				"operations", ops, "err", err)
		}

		return errors.Wrap(err, "failed to run inference")
	}

	s.app.Event.Emit(EventAppExport, ExportUpdate{Hash: file.Hash, State: "RUNNING", Value: progressInferEnd})
	return s.saveAndEmit(ctx, outputData.Pixels, outputPath, overwrite, format, file.Hash)
}

func (s *ImageService) saveAndEmit(
	ctx context.Context,
	pixels image.Image,
	outputPath string,
	overwrite bool,
	format types.ImageFormat,
	fileHash string,
) error {
	if err := ctx.Err(); err != nil {
		slog.Info("export save cancelled", "hash", fileHash)
		return errors.Wrap(err, "context cancelled")
	}

	finalPath := getOutputPath(outputPath, overwrite)
	size, err := utils.SaveImage(&types.ImageData{
		FilePath: finalPath,
		Pixels:   pixels,
	}, format, exportMaxQuality)
	if err != nil {
		s.otel.LogError("Error saving image", nil, err)
		slog.Error("error saving image", "output_path", finalPath, "err", err)
		return errors.Wrap(err, "failed to save image")
	}

	slog.Info("image saved", "output_path", finalPath, "size", size)
	s.app.Event.Emit(EventAppExport, ExportUpdate{Hash: fileHash, State: "COMPLETED", Value: float64(size)})
	return nil
}

// region - Private methods

func (s *ImageService) runInference(
	ctx context.Context,
	filePath string,
	ep types.ExecutionProvider,
	params guitypes.InferenceParams,
	opIds []string,
) (*types.ImageData, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inputImage, err := utils.LoadImage(filePath)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load image")
	}

	operations, err := guiutils.IdsToOperations(opIds, params)
	if err != nil {
		return nil, errors.Wrap(err, "failed to parse operation IDs")
	}

	outputData, err := opai.Process(ctx, inputImage, ep, func(name string, progress float64) {
		s.app.Event.Emit(EventAppProgress, InferenceProgress{Name: name, Progress: progress})
	}, operations...)

	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	return outputData, nil
}

func (s *ImageService) destroy() {}

// endregion

// region - Private functions

func getOutputPath(filePath string, overwrite bool) string {
	if overwrite || !fs.FileExists(filePath) {
		return filePath
	}

	ext := filepath.Ext(filePath)
	basePath := filePath[:len(filePath)-len(ext)]

	for count := 1; count <= maxOutputDedupTries; count++ {
		candidate := fmt.Sprintf("%s_%d%s", basePath, count, ext)
		if !fs.FileExists(candidate) {
			return candidate
		}
	}

	// Exhausted the dedup suffix range; fall back to the last candidate and let the caller's
	// write fail loudly rather than looping forever.
	return fmt.Sprintf("%s_%d%s", basePath, maxOutputDedupTries, ext)
}

// endregion
