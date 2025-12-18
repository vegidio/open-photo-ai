package opai

import (
	"context"

	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/types"
)

// SuggestEnhancements analyzes the input image and returns a list of recommended enhancement operations.
//
// It evaluates the image for potential face recovery and upscaling improvements based on image characteristics such as
// detected faces and resolution.
func SuggestEnhancements(input *types.ImageData) ([]types.Operation, error) {
	operations := make([]types.Operation, 0)

	if frOp, err := analyseFaceRecovery(input); err == nil {
		operations = append(operations, frOp...)
	}
	if upOp, err := analyseUpscale(input); err == nil {
		operations = append(operations, upOp...)
	}

	return operations, nil
}

// region - Private functions

func analyseFaceRecovery(input *types.ImageData) ([]types.Operation, error) {
	operation := make([]types.Operation, 0)

	model, err := facerecovery.GetFdModel()
	if err != nil {
		return nil, err
	}

	faces, err := facerecovery.ExtractFaces(context.Background(), model, input, nil)
	if err != nil {
		return nil, err
	}

	if len(faces) > 0 {
		operation = append(operation, athens.Op(types.PrecisionFp32))
	}

	return operation, nil
}

func analyseUpscale(input *types.ImageData) ([]types.Operation, error) {
	const _1Mp = 1_048_576
	const _2Mp = 4_194_304

	operation := make([]types.Operation, 0)
	bounds := input.Pixels.Bounds()
	mp := bounds.Dx() * bounds.Dy()

	switch {
	case mp <= _1Mp:
		operation = append(operation, kyoto.Op(kyoto.ModeGeneral, 4, types.PrecisionFp32))
	case mp <= _2Mp:
		operation = append(operation, kyoto.Op(kyoto.ModeGeneral, 2, types.PrecisionFp32))
	}

	return operation, nil
}

// endregion
