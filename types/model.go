package types

// Model defines the interface for AI models that process images.
// It encapsulates specific AI models for image processing tasks such as upscaling, enhancement, or other
// transformations.
//
// Implementations should ensure that Destroy is called when the model is no longer needed to prevent resource leaks.
type Model interface {
	// Id returns a unique identifier for the model
	Id() string

	// Name returns a human-readable name for the model
	Name() string

	// Run processes the input image data and returns the processed output
	Run(input *InputData, onProgress func(float32)) (output *OutputData, err error)

	// Destroy cleans up resources and releases memory used by the model
	Destroy()
}
