package facerecovery

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// maskBlurSigma is the Gaussian blur sigma applied to the feathered circular blend mask.
const maskBlurSigma = 15.0

func RestoreFaces(
	ctx context.Context,
	session *utils.Session,
	img image.Image,
	faces []detection.Face,
	variant *Variant,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	// Nothing to restore; return the image untouched (also avoids a divide-by-zero in the progress step below).
	if len(faces) == 0 {
		return img, nil
	}

	tileSize := variant.TileSize
	fidelity := variant.Fidelity
	mask := variant.blendMask()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}
	if onProgress != nil {
		onProgress(0.2)
	}

	total := 0.2
	step := 0.8 / float64(len(faces)*2)

	// One clone for the whole run: every face is then composited into this buffer in place. The caller's image is
	// never written to, which is the guarantee the clone is here for - it just no longer costs a full-frame copy per
	// face to keep it.
	result := imaging.Clone(img)

	for i, face := range faces {
		if err := ctx.Err(); err != nil {
			return nil, errors.Wrap(err, "context cancelled")
		}

		restored, transform, err := restoreSingleFace(session, img, face, tileSize, fidelity)
		if err != nil {
			// The whole operation aborts on the first face that fails, and the error says nothing about which of them
			// it was - which is the only useful thing to know when one particular photo will not process.
			internal.Log().Warn("failed to restore a face", "face", i+1, "of", len(faces), "err", err)
			return nil, errors.Wrap(err, "failed to restore face")
		}

		if err = ctx.Err(); err != nil {
			return nil, errors.Wrap(err, "context cancelled")
		}
		if onProgress != nil {
			total += step
			onProgress(total)
		}

		blendFaceInto(result, restored, mask, transform, face.BoundingBox, tileSize)

		if onProgress != nil {
			total += step
			onProgress(utils.ClampProgress(total))
		}
	}

	return result, nil
}

func restoreSingleFace(
	session *utils.Session,
	img image.Image,
	face detection.Face,
	tileSize int,
	fidelity float32,
) (image.Image, AffineMatrix, error) {
	aligned, transform := alignFace(img, face.Landmarks, tileSize)

	restored, err := runInference(session, aligned, tileSize, fidelity)
	if err != nil {
		return nil, transform, errors.Wrap(err, "failed to run inference")
	}

	return restored, transform, nil
}

// runInference runs face recovery inference on an aligned face image.
//
// A non-negative fidelity is passed as a second input tensor; a negative one selects the single-input path, for the
// models whose graph has no fidelity input.
func runInference(session *utils.Session, aligned image.Image, tileSize int, fidelity float32) (image.Image, error) {
	inputData := utils.ImageToCHW(aligned, false, true)

	shape := ort.NewShape(1, 3, int64(tileSize), int64(tileSize))

	inputTensor, err := ort.NewTensor(shape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	inputs := []ort.Value{inputTensor}

	if fidelity >= 0 {
		weightTensor, wErr := ort.NewTensor(ort.NewShape(1), []float32{fidelity})
		if wErr != nil {
			return nil, errors.Wrap(wErr, "failed to create weight tensor")
		}
		defer weightTensor.Destroy()

		inputs = append(inputs, weightTensor)
	}

	outputData, err := utils.RunSession(session, inputs, shape)
	if err != nil {
		return nil, err
	}

	return utils.CHWToImage(outputData, tileSize, tileSize, true), nil
}
