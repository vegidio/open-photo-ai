package types

import (
	"context"
	"image"
)

// Model is one AI model - an upscaler, a denoiser, a face detector - producing T from an image.
//
// Whoever holds a model is responsible for calling Destroy when it is no longer needed; within this library that is
// the model registry, not the caller.
type Model[T any] interface {
	// Destroyable is a workaround for Destroy() in generic interfaces
	Destroyable

	Id() string

	// Name returns the model's display name, e.g. "Denoise (FP16)".
	Name() string

	// Run processes the image and returns the processed output.
	//
	// params carries operation-specific inputs that are not part of the operation's identity (and therefore are not
	// encoded in Id()), supplied fresh on every call so that registry-cached models never read stale values. Models
	// whose inputs are fully described by their Id ignore it. See Operation.Params (the Parameterized interface).
	Run(ctx context.Context, img image.Image, params map[string]any, onProgress InferenceProgress) (T, error)
}

// Measurable is implemented by models that can report the size of the files backing them, which is what the model
// registry budgets against when deciding how much can stay resident at once.
//
// It is optional: a model that doesn't implement it is charged a conservative default rather than being treated as
// free, so forgetting the method makes the budget coarser instead of defeating it.
type Measurable interface {
	// ResidentBytes returns the on-disk size of the files backing the model
	ResidentBytes() int64
}

// Destroyable releases the native resources behind a value.
//
// It is a named interface rather than a method on Model[T] because Go's type system requires explicit interface
// embedding in a generic interface rather than direct method inclusion.
type Destroyable interface {
	Destroy()
}
