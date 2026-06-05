package facerecovery

import (
	"context"
	"fmt"
	"image"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
)

// ParamFaces is the Model.Run params key under which the pre-detected faces are passed to the face-recovery models.
// Face detection runs independently (see opai.Execute with newyork.Op); the resulting []detection.Face is carried
// on the face-recovery operation and forwarded to Run via this key.
const ParamFaces = "faces"

// FacesCacheKey builds a stable, order-sensitive signature of the faces' bounding boxes, used by the face-recovery
// operations' CacheKey so that changing which faces are recovered invalidates the cached output. Bounding boxes
// uniquely identify the deterministically detected faces within an image, so the signature distinguishes every distinct
// selection while staying identical across re-runs of the same selection. Returns "" when there are no faces.
func FacesCacheKey(faces []detection.Face) string {
	if len(faces) == 0 {
		return ""
	}

	var b strings.Builder
	for _, f := range faces {
		bb := f.BoundingBox
		fmt.Fprintf(&b, "%.2f,%.2f,%.2f,%.2f;", bb.Min.X, bb.Min.Y, bb.Max.X, bb.Max.Y)
	}

	return b.String()
}

func LoadModel(
	ctx context.Context,
	operation types.Operation,
	onProgress types.DownloadProgress,
) (string, error) {
	modelFile := operation.Id() + ".onnx"

	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: operation.Hash(),
	}

	if err := utils.PrepareDependency(ctx, url, "models", fileCheck, onProgress); err != nil {
		return "", errors.Wrapf(err, "failed to prepare Face Recovery model %s", operation.Id())
	}

	return modelFile, nil
}

func ExtractFaces(
	ctx context.Context,
	dtModel types.Model[[]detection.Face],
	img image.Image,
	onProgress types.InferenceProgress,
) ([]detection.Face, error) {
	if onProgress != nil {
		onProgress("fr", 0)
	}

	faces, err := dtModel.Run(ctx, img, nil, nil)
	if err != nil {
		return nil, errors.Wrap(err, "failed to run Face Detection model")
	}

	if onProgress != nil {
		onProgress("fr", 0.1)
	}

	return faces, nil
}
