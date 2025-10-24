package openphotoai

import (
	"fmt"
	"strings"

	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// Registry where all loaded models are stored
var registry = make(map[string]types.Model)

// Execute processes an image through a sequence of operations.
//
// The function selects the appropriate AI model for each operation and runs its inference on the current image data. If
// multiple operations are provided, they are applied in the order specified.
//
// # Parameters:
//   - input: The input image data to be processed
//   - operations: A variable number of operations to apply sequentially
//
// # Returns:
//   - *types.OutputData: The final processed image data after all operations
//   - error: An error if model selection fails, any operation fails, or no operations are provided
//
// Example:
//
//	output, err := Execute(inputData, upscaleOp, denoiseOp)
func Execute(input *types.InputData, operations ...types.Operation) (*types.OutputData, error) {
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

// CleanRegistry releases all resources held by registered models. It iterates through all models in the registry and
// calls their Destroy method to clean up memory and other resources.
//
// This function should be called when the application is shutting down or when all model instances are no longer needed
// to prevent resource leaks.
func CleanRegistry() {
	for _, model := range registry {
		model.Destroy()
	}
}

// region - Private functions

func selectModel(operation types.Operation) (types.Model, error) {
	var model types.Model
	var err error

	model, exists := registry[operation.Id()]
	if exists {
		return model, nil
	}

	switch {
	case strings.HasPrefix(operation.Id(), "upscale"):
		model, err = upscale.New(appName, operation)
	default:
		err = fmt.Errorf("no model found for operation: %s", operation.Id())
	}

	if model != nil {
		registry[operation.Id()] = model
	}

	return model, err
}

// endregion
