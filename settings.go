package opai

import (
	"log/slog"

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

// SetLogger activates structured logging for the OPAI library.
//
// By default, the library is completely silent — it logs nothing and creates no files. Pass a *slog.Logger to receive
// INFO/WARN/ERROR (and DEBUG) events from the library's internals, which is useful for debugging. Applications
// typically don't call this directly; the bundled binaries wire it up through shared.SetupLogging.
func SetLogger(l *slog.Logger) {
	internal.SetLogger(l)
}

// SetSkipModelVerification enables a debug mode where models already present on disk are used as-is.
//
// Normally a locally-present model whose SHA-256 doesn't match the expected Hugging Face hash (or an
// empty/corrupt file) is re-downloaded. With this enabled, a model is downloaded ONLY when it is
// missing — a different hash or an empty file no longer triggers a re-download. Intended for
// debugging with hand-built/experimental models; leave it off in production. Can be called any time
// after Initialize.
func SetSkipModelVerification(skip bool) {
	internal.SetSkipModelVerification(skip)
}
