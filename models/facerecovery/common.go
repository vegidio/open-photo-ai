package facerecovery

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/types"
)

// ParamFaces is the Model.Run params key under which the pre-detected faces are passed to the face-recovery models.
// Face detection runs independently (see opai.Execute with newyork.Op); the resulting []facedetection.Face is carried
// on the face-recovery operation and forwarded to Run via this key.
const ParamFaces = "faces"

func LoadModel(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
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
	fdModel types.Model[[]facedetection.Face],
	img image.Image,
	onProgress types.InferenceProgress,
) ([]facedetection.Face, error) {
	if onProgress != nil {
		onProgress("fr", 0)
	}

	faces, err := fdModel.Run(ctx, img, nil, nil)
	if err != nil {
		return nil, errors.Wrap(err, "failed to run Face Detection model")
	}

	if onProgress != nil {
		onProgress("fr", 0.1)
	}

	return faces, nil
}
