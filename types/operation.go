package types

// Operation defines the interface for image processing operations.
//
// It provides a common abstraction for different types of operations that can be performed on images.
type Operation interface {
	// Id returns a unique identifier for the operation type.
	Id() string

	// Precision returns the precision of the operation.
	Precision() Precision

	// Hash returns a hash of the model used.
	Hash() string
}
