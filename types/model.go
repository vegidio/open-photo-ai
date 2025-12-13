package types

import "context"

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

	// Run processes the input image data and returns the processed output
	Run(ctx context.Context, input *ImageData, onProgress ProgressCallback) (T, error)
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

// ProgressCallback is a function type for reporting progress during model operations.
//
// The operation parameter describes the current processing step, and progress represents the completion percentage as a
// value between 0.0 and 1.0.
type ProgressCallback func(operation string, progress float64)
