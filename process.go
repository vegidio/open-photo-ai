package openphotoai

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/models/upscale"
	"github.com/vegidio/open-photo-ai/internal/types"
)

func Process(input types.InputData, operations ...types.Operation) (*types.OuputData, error) {
	var output *types.OuputData

	for _, op := range operations {
		model, err := selectModel(op)
		if err != nil {
			return nil, err
		}

		if !model.IsLoaded() {
			model.Load()
		}

		output, err = model.Run(op, input)
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
	switch operation.Id() {
	case "upscale":
		return upscale.New(AppName), nil
	}

	return nil, fmt.Errorf("no model found for operation: %s", operation.Id())
}

// endregion
