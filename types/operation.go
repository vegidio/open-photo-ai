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

// Parameterized is an optional interface implemented by operations that carry per-run inputs which are not part of
// their identity (and so are not encoded in Id()). The inference pipeline forwards the returned map to Model.Run on
// every call. Operations whose inputs are fully described by their Id do not implement this.
type Parameterized interface {
	// Params returns the operation's per-run inputs, keyed by name.
	Params() map[string]any
}

// CacheKeyer is an optional interface implemented by operations whose per-run inputs (carried via Parameterized) change
// the produced image and therefore must participate in the output cache key. Without it, two runs of the same Id with
// different inputs (e.g. a different set of selected faces) would collide in the image cache and return stale output.
//
// The returned string only needs to be stable and distinct per distinct input set; return "" when there is nothing to
// contribute (the key then falls back to Id() alone).
type CacheKeyer interface {
	// CacheKey returns a stable signature of the per-run inputs that affect the produced image.
	CacheKey() string
}
