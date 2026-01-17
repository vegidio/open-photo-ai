package facerecovery

import (
	"context"
	"fmt"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/types"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

func LoadModel(
	operation types.Operation,
	onProgress types.DownloadProgress,
) (types.Model[[]facedetection.Face], string, string, error) {
	fdModel, err := GetFdModel()
	if err != nil {
		return nil, "", "", err
	}

	modelFile := operation.Id() + ".onnx"
	modelName := fmt.Sprintf("Face Recovery (%s)",
		cases.Upper(language.English).String(string(operation.Precision())),
	)

	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: operation.Hash(),
	}

	if err = utils.PrepareDependency(url, "models", fileCheck, onProgress); err != nil {
		return nil, "", "", err
	}

	return fdModel, modelFile, modelName, nil
}

func ExtractFaces(
	ctx context.Context,
	fdModel types.Model[[]facedetection.Face],
	input *types.ImageData,
	onProgress types.InferenceProgress,
) ([]facedetection.Face, error) {
	if onProgress != nil {
		onProgress("fr", 0)
	}

	faces, err := fdModel.Run(ctx, input, nil)
	if err != nil {
		return nil, err
	}

	if onProgress != nil {
		onProgress("fr", 0.1)
	}

	return faces, nil
}
