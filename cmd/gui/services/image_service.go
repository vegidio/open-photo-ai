package services

import (
	"context"
	"fmt"
	guitypes "gui/types"
	guiutils "gui/utils"
	"image"
	"log/slog"
	"os"
	"path/filepath"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
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

// GetImage loads an image and returns it JPEG-encoded, together with its width and height.
//
// size caps the longest dimension, resizing proportionally; 0 returns the original dimensions. crop is only applied
// when size == 0 (the full-resolution preview), since the thumbnails are never cropped.
func (s *ImageService) GetImage(filePath string, size int, crop guitypes.CropInfo) ([]byte, int, int, error) {
	inputData, err := utils.LoadImage(filePath)
	if err != nil {
		s.otel.LogError("Error loading image", nil, err)
		slog.Error("error loading image", "file_path", filePath, "err", err)
		return nil, 0, 0, errors.Wrap(err, "failed to load image")
	}

	if size == 0 {
		inputData.Pixels = guiutils.ApplyCropInfo(inputData.Pixels, crop)
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

// ProcessImage runs the operations named by opIds (see utils.IdsToOperations for the ID format) and returns the
// result JPEG-encoded for preview, together with its width and height.
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

	bounds := outputData.Pixels.Bounds()
	return data, bounds.Dx(), bounds.Dy(), nil
}

// SuggestEnhancements analyzes an image and returns the enhancement types worth applying to it.
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

// ExportImage runs the operations named by opIds and writes the result to outputPath at full quality. Unless
// overwrite is set, an existing file is left alone and a "_N" suffix is added instead (see getOutputPath).
//
// Progress is reported through EventAppExport rather than the return value, keyed by file.Hash.
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
		// Cancellation is expected (user canceled the export) — log it as info, not an error.
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

	finalPath, release := getOutputPath(outputPath, overwrite)
	size, err := utils.SaveImage(&types.ImageData{
		FilePath: finalPath,
		Pixels:   pixels,
	}, format, exportMaxQuality)
	if err != nil {
		// The name was claimed before encoding; drop the empty placeholder so a failed export leaves nothing behind.
		release()
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

	// Apply the user's flip/rotate/crop before any enhancement runs (no-op when no crop was set). The crop changes the
	// pixels but not the file hash, so fold a crop signature into the hash — otherwise opai.Process's per-operation
	// cache (keyed by input hash) would return a stale uncropped result.
	inputImage.Pixels = guiutils.ApplyCropInfo(inputImage.Pixels, params.Crop)
	inputImage.Hash += guiutils.CropCacheKey(params.Crop)

	operations, err := guiutils.IdsToOperations(opIds, params)
	if err != nil {
		return nil, errors.Wrap(err, "failed to parse operation IDs")
	}

	outputData, err := opai.Process(ctx, inputImage, ep, func(progress types.Progress) {
		s.app.Event.Emit(EventAppProgress, InferenceProgress{
			Name:     progress.Operation,
			Phase:    progress.Phase,
			Progress: progress.Total,
			Fraction: progress.Fraction,
		})
	}, operations...)

	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	return outputData, nil
}

func (s *ImageService) destroy() {}

// endregion

// region - Private functions

// getOutputPath resolves the file an export should write to, appending a "_N" suffix when the requested path is taken
// and overwrite is off. It returns the path plus a release func the caller must invoke if the write never happens.
//
// Each candidate is claimed by creating it with O_EXCL rather than probing with a stat first: two exports of different
// images that resolve to the same name would otherwise both observe "_1" as free, and the second write would silently
// destroy the first. Claiming the name makes the filesystem, not a racy check, decide the winner — at the cost of an
// empty placeholder that release removes on the failure path.
func getOutputPath(filePath string, overwrite bool) (string, func()) {
	if overwrite {
		return filePath, func() {}
	}

	if release, ok := claimPath(filePath); ok {
		return filePath, release
	}

	ext := filepath.Ext(filePath)
	basePath := filePath[:len(filePath)-len(ext)]

	for count := 1; count <= maxOutputDedupTries; count++ {
		candidate := fmt.Sprintf("%s_%d%s", basePath, count, ext)
		if release, ok := claimPath(candidate); ok {
			return candidate, release
		}
	}

	// Exhausted the dedup suffix range; fall back to the last candidate and let the caller's
	// write fail loudly rather than looping forever.
	return fmt.Sprintf("%s_%d%s", basePath, maxOutputDedupTries, ext), func() {}
}

// claimPath creates path exclusively, reporting whether this caller now owns the name. Only an already-exists error
// means "taken": any other failure (a missing directory, no write permission) is left for the real write to surface
// with a message about the path the user actually asked for.
func claimPath(path string) (release func(), ok bool) {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
	if err != nil {
		return nil, !errors.Is(err, os.ErrExist)
	}

	_ = f.Close()
	return func() { _ = os.Remove(path) }, true
}

// endregion
