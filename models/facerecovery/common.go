package facerecovery

import (
	"context"
	"fmt"
	"image"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/types"
)

func LoadModel(
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (types.Model[[]facedetection.Face], string, error) {
	fdModel, err := GetFdModel(ep)
	if err != nil {
		return nil, "", err
	}

	modelFile := operation.Id() + ".onnx"

	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: operation.Hash(),
	}

	if err = utils.PrepareDependency(url, "models", fileCheck, onProgress); err != nil {
		return nil, "", err
	}

	return fdModel, modelFile, nil
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

	faces, err := fdModel.Run(ctx, img, nil)
	if err != nil {
		return nil, err
	}

	if onProgress != nil {
		onProgress("fr", 0.1)
	}

	return faces, nil
}
