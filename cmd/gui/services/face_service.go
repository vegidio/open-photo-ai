package services

import (
	"context"
	guitypes "gui/types"
	guiutils "gui/utils"
	"log/slog"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/o11y"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/detection/newyork"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
	"github.com/wailsapp/wails/v3/pkg/application"
)

type FaceService struct {
	app  *application.App
	otel *o11y.Telemetry
}

func NewFaceService(app *application.App, otel *o11y.Telemetry) *FaceService {
	return &FaceService{
		app:  app,
		otel: otel,
	}
}

// DetectFaces runs the face-detection model on an image and returns the detected faces.
//
// The frontend calls this independently and then passes the result back to ProcessImage/ExportImage so that face
// recovery no longer triggers detection internally. The crop is applied (flip→rotate→crop) before detection so the
// resulting bounding boxes live in the cropped image's coordinate space — matching the cropped source that face
// recovery and the preview operate on. Results are deterministic for a given image+crop+precision, so the frontend
// caches them by file hash plus a crop token plus the precision.
//
// precision is the one the face-recovery operation these faces are for was built at, so a recovery on the SD tier
// detects with the fp16 graph and one on HD with the fp32 graph, instead of every run pulling the fp32 graph down to
// feed whichever the user picked.
func (s *FaceService) DetectFaces(
	ctx context.Context,
	filePath string,
	ep types.ExecutionProvider,
	precision types.Precision,
	crop guitypes.CropInfo,
) ([]detection.Face, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inputImage, err := utils.LoadImage(filePath)
	if err != nil {
		s.otel.LogError("Error loading image", nil, err)
		slog.Error("error loading image", "file_path", filePath, "err", err)
		return nil, errors.Wrap(err, "failed to load image")
	}

	// Detect on the cropped image so faces share the same coordinate space as the cropped source used by face
	// recovery; fold the crop into the hash for parity with runInference's cache safety.
	inputImage.Pixels = guiutils.ApplyCropInfo(inputImage.Pixels, crop)
	inputImage.Hash += guiutils.CropCacheKey(crop)

	faces, err := opai.Execute[[]detection.Face](ctx, inputImage, ep, nil, newyork.Op(precision))
	if err != nil {
		// Cancellation is expected (user navigated away / cancelled) — log it as info, not an error.
		if errors.Is(err, context.Canceled) {
			slog.Info("face detection cancelled", "file_path", filePath)
		} else {
			s.otel.LogError("Error detecting faces", nil, err)
			slog.Error("error detecting faces", "file_path", filePath, "err", err)
		}

		return nil, errors.Wrap(err, "failed to detect faces")
	}

	slog.Info("faces detected", "file_path", filePath, "count", len(faces))
	return faces, nil
}

func (s *FaceService) destroy() {}
