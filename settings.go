package opai

import (
	"log/slog"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// ModelMemoryStats reports how much memory the loaded models are holding, per pool, along with the ceiling each pool
// is being kept under.
//
// The numbers are the on-disk size of the model files, which is a proxy rather than a measurement: the real footprint
// is larger, because memory arenas, cuDNN workspaces and the CoreML MLProgram all sit on top of the weights and none
// of them is queryable through the ONNX bindings. That is why the default budgets leave a wide margin. Useful for a
// diagnostics screen or a benchmark header; not something an application needs to consult to work correctly.
func ModelMemoryStats() types.ModelMemory {
	return internal.Registry.Stats()
}

// ResetProviderFallback forgets that an execution provider was found to be unusable, so the next model built with it
// tries it again instead of going straight to the CPU.
//
// The library latches that failure to avoid paying a full provider initialization plus a failing session build for
// every model in a run once a driver has proved broken. The latch should be cleared when the user does something that
// means "try again" - picking a provider in the settings, typically after installing a driver - which is why it is an
// explicit call rather than something tied to memory being freed.
func ResetProviderFallback() {
	internal.ResetFallback()
}

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

// SetModelBudget caps how much memory the models loaded in one pool may occupy. Passing 0 makes that pool unbounded.
//
// Each pool defaults to a fraction of what the machine has - a share of VRAM for the device pool, a share of system
// RAM less a reserve for the image pipeline for the host pool - computed once during Initialize. Raising a budget lets
// more models stay resident, which trades memory for not rebuilding sessions; lowering it does the reverse. Lowering
// it evicts nothing immediately: the new ceiling applies to the next model admitted, so a settings change never stalls
// work already running.
//
// A model larger than the whole budget is still loaded, after everything idle in its pool has been evicted to make
// room. A budget is a target, not a hard limit that can refuse to run an operation the user asked for.
func SetModelBudget(pool types.MemoryPool, bytes int64) {
	internal.Registry.SetBudget(pool, bytes)
}

// SetModelIdleTTL sets how long a model stays loaded after nothing is using it. Passing 0 disables the idle sweep,
// leaving the memory budget as the only thing that unloads models.
//
// Models are kept resident between operations because building an ONNX session is expensive - full graph optimization,
// plus the provider's own compilation, which for TensorRT means building an engine. The TTL is what eventually gives
// that memory back when the user has moved on. It is deliberately longer than any gap within a batch export, so a
// batch never rebuilds a model it is still working through.
func SetModelIdleTTL(ttl time.Duration) {
	internal.Registry.SetIdleTTL(ttl)
}

// SetSkipModelVerification enables a debug mode where models already present on disk are used as-is.
//
// Normally a locally present model whose SHA-256 doesn't match the expected Hugging Face hash (or an
// empty/corrupt file) is re-downloaded. With this enabled, a model is downloaded ONLY when it is
// missing — a different hash or an empty file no longer triggers a re-download. Intended for
// debugging with hand-built/experimental models; leave it off in production. Can be called any time
// after Initialize.
func SetSkipModelVerification(skip bool) {
	internal.SetSkipModelVerification(skip)
}
