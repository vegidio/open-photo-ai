package opai

import (
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// SetFallbackHandler registers a function to be called whenever inference falls back to the CPU because the requested
// execution provider (CUDA, TensorRT, CoreML, …) failed to create a model. It's meant for surfacing the downgrade to
// the user, since CPU inference is noticeably slower. Passing nil removes a previously registered handler.
//
// The handler is called from the goroutine running the inference, and only for the first model that had to be
// downgraded: once a provider is known to be unusable the remaining models are built on the CPU directly, without
// another failed attempt to report. Cleaning the registry resets that, so a downgrade is reported again after the user
// picks a different provider.
func SetFallbackHandler(handler types.FallbackHandler) {
	internal.SetFallbackHandler(handler)
}
