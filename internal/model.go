package internal

import (
	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/types"
)

// maxAcquireAttempts bounds the retry loop in AcquireModel. It is a livelock backstop, not a tuning knob: reaching it
// means something is destroying models as fast as they can be built.
//
// A caller that leads a build never retries - it takes its lease inside install, while the registry lock is still
// held, so nothing can remove the entry before it is pinned. Only a caller that waited on someone else's build can
// come back around, and only if that model was removed again before the waiter re-looked. Each retry is therefore a
// real, if unlucky, sequence of events rather than a spin.
//
// The bound is generous because the cost of being wrong is asymmetric. Retrying a few extra times is invisible;
// returning an error means an enhancement fails in front of the user for a reason they cannot act on.
const maxAcquireAttempts = 10

// AcquireModel returns a lease on the model registered under id, building it on first use.
//
// The caller MUST release the lease when it is done with the model, with a deferred call placed immediately after the
// error check. Until it does, the model is pinned: nothing can destroy it, so inference is safe without holding any
// process-wide lock.
//
// Creations are single-flighted per id. Concurrent callers for the same model wait for the first one instead of each
// building their own - which also closes a leak that the previous get-or-create had, where two racing builds both
// stored into the registry and the loser's ONNX session was orphaned and never destroyed.
//
// A GPU execution provider can fail for reasons outside the application's control: an outdated or broken driver, a GPU
// that is unavailable, or one without enough free memory. When that happens the model is rebuilt on the CPU, which
// keeps the app usable (just slower) instead of failing the whole operation, and the downgrade is reported through the
// handler registered with SetFallbackHandler. Failures that aren't about the execution provider - a model that
// couldn't be downloaded, for instance - would fail the same way on the CPU, so they aren't retried.
//
// This is the single acquire path for every model, so the operations pipeline and the face detection behind
// SuggestEnhancements can't drift apart on caching or fallback behaviour.
func AcquireModel(
	id string,
	requested types.ExecutionProvider,
	create func(ep types.ExecutionProvider) (any, error),
) (*Lease, error) {
	for range maxAcquireAttempts {
		// Resolve the provider before keying, so the entry is filed under the one the model will actually be built on.
		// Keying by the requested provider instead would file a CPU model under "@CUDA" after a fallback, and an
		// explicit switch to CPU would then build a second, identical copy of it.
		//
		// Resolved once per attempt rather than again at the build below: the latch it reads can flip between the two
		// reads, which would key the entry under one provider and build it on another.
		ep := effectiveProvider(requested)
		key := registryKey(id, ep)

		lease, b, leader, err := Registry.acquireOrBuild(key)
		if err != nil {
			return nil, err
		}

		if lease != nil {
			Log().Debug("model registry hit", "op", id)
			return lease, nil
		}

		if !leader {
			// Someone else is already building this model. Wait for them, then re-run the loop rather than inheriting
			// their entry: by the time we wake up it may already have been removed again, and only a fresh lookup can
			// tell. That also makes a leader that fell back to the CPU need no special handling here.
			<-b.done
			if b.err != nil {
				return nil, b.err
			}

			continue
		}

		return buildAndInstall(id, key, ep, create)
	}

	return nil, errors.Newf("model registry churn: gave up acquiring %s", id)
}

// registryKey identifies one resident model. The execution provider is part of it because the same operation built on
// two different providers is two different models holding two different sets of native resources.
//
// Keying this way is what lets a change of processor be an ordinary cache miss - the new provider simply isn't in the
// registry yet - rather than something that has to stop the world and destroy every loaded model first.
func registryKey(id string, ep types.ExecutionProvider) string {
	return id + "@" + string(ep)
}

// effectiveProvider resolves what a request for ep will actually be built on.
//
// Once a provider has proved unusable in this run, everything asking for it goes straight to the CPU: the outcome
// won't change until the driver situation does, and each attempt otherwise costs a full provider initialization plus a
// failing session build. Applying the latch here, before the key is formed, keeps the registry honest - an entry is
// always filed under the provider its model was really built on.
func effectiveProvider(ep types.ExecutionProvider) types.ExecutionProvider {
	if latched := failedProvider.Load(); latched != nil && *latched == ep {
		return types.ExecutionProviderCPU
	}

	return ep
}

// buildAndInstall is the leader half of AcquireModel: it owns the pending build registered under buildKey and must
// always resolve it, whether it succeeds or fails, or every waiter blocks forever.
func buildAndInstall(
	id, buildKey string,
	ep types.ExecutionProvider,
	create func(ep types.ExecutionProvider) (any, error),
) (*Lease, error) {
	// Free memory before building rather than after. The estimate comes from the download manifest and is 0 for
	// anything it can't name, which must be read as "unknown" - the real size is charged once the model exists, and
	// install trims then if this was too optimistic.
	pool := PoolOf(ep)
	estimate := EstimateModelBytes(id)
	DestroyEntries(Registry.makeRoom(pool, estimate))

	Log().Info("creating model", "op", id, "ep", ep)
	model, err := create(ep)

	if err != nil && ep != types.ExecutionProviderCPU && errors.Is(err, ErrCreateSession) {
		Log().Warn("model creation failed; retrying on CPU", "op", id, "ep", ep, "err", err)

		cpuModel, cpuErr := create(types.ExecutionProviderCPU)

		// Only report the downgrade once it actually worked; if the CPU fails too, the caller surfaces the error.
		if cpuErr == nil {
			// Latch a copy, not &ep: ep is reassigned on the next line, and storing its address would leave the latch
			// pointing at "CPU" - which would then send every CPU request down the fallback path.
			failed := ep
			failedProvider.Store(&failed)
			notifyFallback(failed, err)

			ep = types.ExecutionProviderCPU
		}

		model, err = cpuModel, cpuErr
	}

	// We can't check `model != nil` here because model is an interface and in Go a variable is only nil if both its
	// type and value are nil. In this case, even though the value is nil, the variable has a concrete type.
	if err != nil {
		Log().Warn("model creation failed", "op", id, "ep", ep, "err", err)
		Registry.releaseReservation(pool, estimate)
		Registry.resolveBuild(buildKey, err)

		return nil, err
	}

	// A fallback moves the model to a different pool than the one reserved against, so give the reservation back where
	// it was taken before charging the model where it actually landed.
	landed := PoolOf(ep)
	if landed != pool {
		Registry.releaseReservation(pool, estimate)
		estimate = 0
	}

	// After a fallback this is not buildKey: the model is filed under the CPU. Waiters re-resolve the provider when
	// they wake, see the latch, and look up that same CPU key - so the move needs no hand-off.
	lease, dup, trim := Registry.install(registryKey(id, ep), id, ep, landed, model, residentBytes(model), estimate)
	Registry.resolveBuild(buildKey, nil)

	// Evicted because this model turned out bigger than its estimate.
	DestroyEntries(trim)

	// Lost an install race: the incumbent is the one everyone else is already using, so this copy is dead on arrival.
	if dup != nil {
		Log().Debug("discarding duplicate model built in a race", "op", id)
		destroyModel(dup)
	}

	return lease, nil
}
