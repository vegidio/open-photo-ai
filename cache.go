package opai

import "github.com/vegidio/open-photo-ai/internal"

// SetImageCacheEnabled turns the per-operation image cache on or off. It is enabled by default.
//
// Process memoizes the result of every operation on disk, keyed by the input image's hash plus the operations applied
// so far, so repeating a sequence the user has already run returns immediately instead of re-running inference. The
// write side of that is not free: the result is PNG-encoded and written to disk inside the Process call, which for a
// large upscale is a substantial fraction of the call's cost.
//
// Disabling it means every Process call re-runs inference and nothing is written to disk. That is what a benchmark
// wants — otherwise it measures a cache read, or charges the PNG encode to the model — and it is also useful for an
// embedder that keeps its own cache, or that processes images it must not persist. Can be called any time after
// Initialize; it takes effect on the next Process call.
func SetImageCacheEnabled(enabled bool) {
	internal.SetImageCacheEnabled(enabled)
}
