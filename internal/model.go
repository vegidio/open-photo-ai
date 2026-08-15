package internal

import (
	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/types"
)

// GetOrCreateModel returns the model registered under id, building it with create and registering it when it isn't
// loaded yet.
//
// A GPU execution provider can fail for reasons outside the application's control: an outdated or broken driver, a GPU
// that is unavailable, or one without enough free memory. When that happens the model is rebuilt on the CPU, which
// keeps the app usable (just slower) instead of failing the whole operation, and the downgrade is reported through the
// handler registered with SetFallbackHandler. Failures that aren't about the execution provider - a model that
// couldn't be downloaded, for instance - would fail the same way on the CPU, so they aren't retried.
//
// This is the single get-or-create path for every model, so the operations pipeline and the face detection behind
// SuggestEnhancements can't drift apart on caching or fallback behaviour.
func GetOrCreateModel(
	id string,
	ep types.ExecutionProvider,
	create func(ep types.ExecutionProvider) (any, error),
) (any, error) {
	if model, exists := Registry.Get(id); exists {
		Log().Debug("model registry hit", "op", id)
		return model, nil
	}

	// Don't re-attempt a provider that already failed in this run: the outcome won't change until the models are
	// rebuilt, and each attempt costs a full provider initialization plus a failing session build.
	if latched := failedProvider.Load(); latched != nil && *latched == ep {
		Log().Debug("execution provider already failed; creating on CPU", "op", id, "ep", ep)
		ep = types.ExecutionProviderCPU
	}

	Log().Info("creating model", "op", id, "ep", ep)
	model, err := create(ep)

	if err != nil && ep != types.ExecutionProviderCPU && errors.Is(err, ErrCreateSession) {
		Log().Warn("model creation failed; retrying on CPU", "op", id, "ep", ep, "err", err)

		cpuModel, cpuErr := create(types.ExecutionProviderCPU)

		// Only report the downgrade once it actually worked; if the CPU fails too, the caller surfaces the error.
		if cpuErr == nil {
			failedProvider.Store(&ep)
			notifyFallback(ep, err)
		}

		model, err = cpuModel, cpuErr
	}

	// We can't check `model != nil` here because model is an interface and in Go a variable is only nil if both its
	// type and value are nil. In this case, even though the value is nil, the variable has a concrete type.
	if err != nil {
		Log().Warn("model creation failed", "op", id, "ep", ep, "err", err)
		return nil, err
	}

	Registry.Set(id, model)
	return model, nil
}
