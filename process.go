package openphotoai

import (
	"fmt"
	"strings"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

func Process(input *types.InputData, operations ...types.Operation) (*types.OutputData, error) {
	var output *types.OutputData

	for _, op := range operations {
		model, err := selectModel(op)
		if err != nil {
			return nil, err
		}

		output, err = model.Run(input)
		if err != nil {
			return nil, err
		}

		// Update the input pixels so that the next operation can use them
		input.Pixels = output.Pixels
	}

	if output == nil {
		return nil, fmt.Errorf("unexpected error: output is nil")
	}

	return output, nil
}

// region - Private functions

func selectModel(operation types.Operation) (types.Model, error) {
	var model types.Model
	var err error

	switch {
	case strings.HasPrefix(operation.Id(), "upscale"):
		model, err = upscale.New(AppName, operation)
	default:
		err = fmt.Errorf("no model found for operation: %s", operation.Id())
	}

	return model, err
}

// endregion
