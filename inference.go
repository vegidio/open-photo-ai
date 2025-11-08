package opai

import (
	"fmt"
	"strings"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

// Process processes an image through a sequence of image operations.
//
// The function selects the appropriate AI model for each operation and runs its inference on the image. If multiple
// operations are provided, they are applied in the order specified. The output is the final processed image after all
// operations are applied.
//
// # Parameters:
//   - input: The input image data to be processed
//   - onProgress: A callback function that is called with the progress of the current operation (0-1)
//   - operations: A variable number of operations to apply sequentially
//
// # Returns:
//   - *types.OutputImage: The final processed image data after all operations
//   - error: An error if model selection fails, any operation fails, or no operations are provided
//
// # Example:
//
//	output, err := Process(inputImage, faceRecoveryOp, upscaleOp)
func Process(input *types.InputImage, onProgress func(float32), operations ...types.Operation) (*types.OutputImage, error) {
	var output *types.OutputImage

	// Make a copy of the input data so the original input is not modified
	inputCopy := &types.InputImage{Pixels: input.Pixels}

	for _, op := range operations {
		model, err := selectModel(op)
		if err != nil {
			return nil, err
		}

		imageModel, ok := model.(types.Model[*types.OutputImage])
		if !ok {
			return nil, fmt.Errorf("operation type not supported: %s", op.Id())
		}

		output, err = imageModel.Run(inputCopy, onProgress)
		if err != nil {
			return nil, err
		}

		// Update the input pixels so that the next operation can use them
		inputCopy.Pixels = output.Pixels
	}

	if output == nil {
		return nil, fmt.Errorf("unexpected error: output is nil")
	}

	return output, nil
}

// Execute executes a single image operation and returns the result as a generic data type.
//
// The function selects the appropriate AI model for the operation and runs its inference on the image. The output is
// not an image, but the information data returned by the model.
//
// # Parameters:
//   - input: The input image data to be processed
//   - onProgress: A callback function that is called with the progress of the current operation (0-1)
//   - operation: The operation to apply to the image
//
// # Returns:
//   - T: The result of the operation with the specified generic type
//   - error: An error if model selection fails, the operation fails, or the operation type is not supported
//
// # Example:
//
//	faces, err := Execute[[]types.Face](inputImage, progressCallback, faceDetectionOp)
func Execute[T any](input *types.InputImage, onProgress func(float32), operation types.Operation) (T, error) {
	// nil value for type T
	var genericNil T

	model, err := selectModel(operation)
	if err != nil {
		return genericNil, err
	}

	dataModel, ok := model.(types.Model[T])
	if !ok {
		return genericNil, fmt.Errorf("operation type not supported: %s", operation.Id())
	}

	return dataModel.Run(input, onProgress)
}

// CleanRegistry releases all resources held by registered models. It iterates through all models in the registry and
// calls their Destroy method to clean up memory and other resources.
//
// This function should be called when the application is shutting down or when all model instances are no longer needed
// to prevent resource leaks.
func CleanRegistry() {
	for _, model := range internal.Registry {
		if destroyable, ok := model.(types.Destroyable); ok {
			destroyable.Destroy()
		}
	}
}

// region - Private functions

func selectModel(operation types.Operation) (interface{}, error) {
	var model interface{}
	var err error

	model, exists := internal.Registry[operation.Id()]
	if exists {
		return model, nil
	}

	switch {
	case strings.HasPrefix(operation.Id(), "face-detection"):
		model, err = facedetection.New(appName, operation)
	case strings.HasPrefix(operation.Id(), "face-recovery"):
		model, err = facerecovery.New(appName, operation)
	case strings.HasPrefix(operation.Id(), "upscale"):
		model, err = upscale.New(appName, operation)
	default:
		err = fmt.Errorf("no model found for operation: %s", operation.Id())
	}

	if model != nil {
		internal.Registry[operation.Id()] = model
	}

	return model, err
}

// endregion
