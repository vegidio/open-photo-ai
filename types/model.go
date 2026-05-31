package types

import (
	"context"
	"image"
)

// Model defines the interface for AI models that process images.
// It encapsulates specific AI models for image processing tasks such as upscaling, enhancement, or other
// transformations.
//
// Implementations should ensure that Destroy is called when the model is no longer needed to prevent resource leaks.
type Model[T any] interface {
	// Destroyable is a workaround for Destroy() in generic interfaces
	Destroyable

	// Id returns a unique identifier for the model
	Id() string

	// Name returns a human-readable name for the model
	Name() string

	// Run processes the image and returns the processed output.
	//
	// params carries operation-specific inputs that are not part of the operation's identity (and therefore are not
	// encoded in Id()), supplied fresh on every call so that registry-cached models never read stale values. Models
	// whose inputs are fully described by their Id ignore it. See Operation.Params (the Parameterized interface).
	Run(ctx context.Context, img image.Image, params map[string]any, onProgress InferenceProgress) (T, error)
}

// Destroyable defines an interface for types that require explicit resource cleanup. Implementations must provide a
// Destroy method to release allocated resources, memory, or handles when the instance is no longer needed.
//
// This interface is used as a workaround to embed cleanup functionality in generic interfaces like Model[T], where Go's
// type system requires explicit interface embedding rather than direct method inclusion.
type Destroyable interface {
	// Destroy cleans up resources and releases memory used by the model
	Destroy()
}
