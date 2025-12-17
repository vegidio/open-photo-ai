package facerecovery

import (
	"context"
	"fmt"
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

func RestoreFaces(
	ctx context.Context,
	session *ort.DynamicAdvancedSession,
	img image.Image,
	faces []facedetection.Face,
	tileSize int,
	fidelity float32,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	mask := createCircularMask(tileSize, tileSize, 15.0)

	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if onProgress != nil {
		onProgress("fr", 0.2)
	}

	total := 0.2
	step := 0.8 / float64(len(faces)*2)

	// This result image will later be cloned in the blendFace function,
	// so there's no risk any changes downstream will affect the original image
	result := img

	for _, face := range faces {
		if err := ctx.Err(); err != nil {
			return nil, err
		}

		restored, transform, err := restoreSingleFace(session, img, face, tileSize, fidelity)
		if err != nil {
			return nil, err
		}

		if err = ctx.Err(); err != nil {
			return nil, err
		}
		if onProgress != nil {
			total += step
			onProgress("fr", total)
		}

		result = blendFace(result, restored, mask, transform, face.BoundingBox, tileSize)

		if onProgress != nil {
			total += step
			onProgress("fr", utils.Ceiling(total))
		}
	}

	return result, nil
}

func restoreSingleFace(
	session *ort.DynamicAdvancedSession,
	img image.Image,
	face facedetection.Face,
	tileSize int,
	fidelity float32,
) (image.Image, AffineMatrix, error) {
	aligned, transform := alignFace(img, face.Landmarks, tileSize)

	restored, err := runInference(session, aligned, tileSize, fidelity)
	if err != nil {
		return nil, transform, err
	}

	return restored, transform, nil
}

// runInference runs face recovery inference on an aligned face image.
//
// If fidelityWeight is positive, it will be used as a second input tensor; otherwise, only the image input will be
// used.
func runInference(session *ort.DynamicAdvancedSession, aligned image.Image, tileSize int, fidelity float32) (image.Image, error) {
	inputData := utils.ImageToCHW(aligned, false, true)

	inputTensor, err := ort.NewTensor(ort.NewShape(1, 3, int64(tileSize), int64(tileSize)), inputData)
	if err != nil {
		return nil, fmt.Errorf("failed to create input tensor: %v", err)
	}
	defer inputTensor.Destroy()

	outputTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, 3, int64(tileSize), int64(tileSize)))
	if err != nil {
		return nil, fmt.Errorf("failed to create output tensor: %v", err)
	}
	defer outputTensor.Destroy()

	// If fidelityWeight is provided, include it as a second input
	if fidelity >= 0 {
		weightTensor, err := ort.NewTensor(ort.NewShape(1), []float32{fidelity})
		if err != nil {
			return nil, fmt.Errorf("failed to create weight tensor: %v", err)
		}
		defer weightTensor.Destroy()

		err = session.Run([]ort.Value{inputTensor, weightTensor}, []ort.Value{outputTensor})
		if err != nil {
			return nil, fmt.Errorf("inference failed: %v", err)
		}
	} else {
		err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor})
		if err != nil {
			return nil, fmt.Errorf("inference failed: %v", err)
		}
	}

	outputData := outputTensor.GetData()
	return utils.CHWToImage(outputData, tileSize, tileSize, true), nil
}
