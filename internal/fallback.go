package internal

import (
	"sync/atomic"

	"github.com/vegidio/open-photo-ai/types"
)

// fallbackHandler is notified whenever a model has to be built on the CPU because the requested execution provider
// couldn't create a session. It defaults to nil so the library reports nothing unless the embedding application opts
// in via opai.SetFallbackHandler.
var fallbackHandler atomic.Pointer[types.FallbackHandler]

// failedProvider latches the execution provider that already proved unusable. The registry is keyed by operation ID,
// so without this a broken driver would cost a failed provider initialization plus a failed session build for every
// single model in a run; with it, the remaining models go straight to the CPU. Cleared by CleanRegistry, so picking a
// different provider gets a fresh attempt.
var failedProvider atomic.Pointer[types.ExecutionProvider]

// SetFallbackHandler swaps the handler called when inference is downgraded to the CPU. A nil handler removes the
// current one. Safe for concurrent use.
func SetFallbackHandler(handler types.FallbackHandler) {
	// Storing &handler when handler is nil would leave a non-nil *FallbackHandler behind, which notifyFallback would
	// happily dereference into a nil func call.
	if handler == nil {
		fallbackHandler.Store(nil)
		return
	}

	fallbackHandler.Store(&handler)
}

// ResetFallback forgets the latched execution provider so the next model creation gives the requested provider another
// chance. Called when the registry is cleaned, which is what happens when the user picks a different provider.
func ResetFallback() {
	failedProvider.Store(nil)
}

// notifyFallback informs the registered handler, if any, that ep was downgraded to the CPU.
func notifyFallback(ep types.ExecutionProvider, err error) {
	if handler := fallbackHandler.Load(); handler != nil {
		(*handler)(ep, err)
	}
}
