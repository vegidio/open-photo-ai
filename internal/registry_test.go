package internal

import (
	"errors"
	"fmt"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/vegidio/open-photo-ai/types"
)

// fakeModel stands in for a real model so the registry can be tested with no ONNX runtime and no GPU.
//
// Destroy and run are the two halves of issue #34: destroying a model twice, or running one after it was destroyed,
// are both native use-after-frees in production. Here they panic, so a test that provokes either one fails loudly
// instead of passing quietly.
type fakeModel struct {
	bytes     int64
	destroyed atomic.Bool
}

func (f *fakeModel) ResidentBytes() int64 { return f.bytes }

func (f *fakeModel) Destroy() {
	if f.destroyed.Swap(true) {
		panic("double destroy")
	}
}

func (f *fakeModel) run() {
	if f.destroyed.Load() {
		panic("use after destroy")
	}
}

// Compile-time assertions that the fake satisfies the interfaces the registry cares about.
var (
	_ types.Measurable  = (*fakeModel)(nil)
	_ types.Destroyable = (*fakeModel)(nil)
)

// withRegistry swaps in a fresh registry for the duration of a test, since Registry is a package-level singleton.
func withRegistry(t *testing.T) {
	t.Helper()

	original := Registry
	Registry = newModelRegistry()
	t.Cleanup(func() { Registry = original })
}

// acquireFake acquires id, counting how many times the model actually had to be built.
func acquireFake(id string, built *atomic.Int64, bytes int64) (*Lease, error) {
	return AcquireModel(id, types.ExecutionProviderCPU, func(types.ExecutionProvider) (any, error) {
		built.Add(1)
		return &fakeModel{bytes: bytes}, nil
	})
}

func TestAcquireHitAndRefcount(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	first, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("first acquire: %v", err)
	}

	second, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("second acquire: %v", err)
	}

	if got := built.Load(); got != 1 {
		t.Errorf("create called %d times, want 1", got)
	}

	if first.Model() != second.Model() {
		t.Error("two leases on the same id returned different models")
	}

	if got := Registry.Leases(); got != 2 {
		t.Errorf("leases = %d, want 2", got)
	}

	first.Release()
	second.Release()

	if got := Registry.Leases(); got != 0 {
		t.Errorf("leases after release = %d, want 0", got)
	}

	// Releasing every lease must NOT evict: refcount zero only makes an entry eligible, which is what keeps a batch
	// export from rebuilding the same model for every image.
	if got := Registry.Len(); got != 1 {
		t.Errorf("resident = %d after releasing all leases, want 1", got)
	}
}

// TestSequentialReuse is the batch-export guarantee: N images with the same enhancement chain must build once.
func TestSequentialReuse(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	for i := range 10 {
		lease, err := acquireFake("op", &built, 10)
		if err != nil {
			t.Fatalf("acquire %d: %v", i, err)
		}

		lease.Model().(*fakeModel).run()
		lease.Release()
	}

	if got := built.Load(); got != 1 {
		t.Errorf("create called %d times over 10 sequential acquires, want 1", got)
	}
}

// TestSingleFlight covers the leak the old get-or-create had: two racing builds both stored into the registry and the
// loser's session was orphaned. Every caller must end up with the same model, and only one may be built.
func TestSingleFlight(t *testing.T) {
	withRegistry(t)

	const goroutines = 50

	var (
		built   atomic.Int64
		start   sync.WaitGroup
		done    sync.WaitGroup
		mu      sync.Mutex
		models  = make(map[any]int)
		release = make([]*Lease, 0, goroutines)
	)

	start.Add(1)

	for range goroutines {

		done.Go(func() {
			start.Wait()

			lease, err := AcquireModel("op", types.ExecutionProviderCPU, func(types.ExecutionProvider) (any, error) {
				built.Add(1)
				return &fakeModel{bytes: 10}, nil
			})

			if err != nil {
				t.Errorf("acquire: %v", err)
				return
			}

			mu.Lock()
			models[lease.Model()]++
			release = append(release, lease)
			mu.Unlock()
		})
	}

	start.Done()
	done.Wait()

	if got := built.Load(); got != 1 {
		t.Errorf("create called %d times under %d concurrent acquires, want 1", got, goroutines)
	}

	if len(models) != 1 {
		t.Errorf("callers saw %d distinct models, want 1", len(models))
	}

	for _, lease := range release {
		lease.Release()
	}
}

// TestSingleFlightPropagatesError checks that waiters fail the same way the leader did, rather than each retrying a
// creation that just proved impossible.
func TestSingleFlightPropagatesError(t *testing.T) {
	withRegistry(t)

	wantErr := errors.New("model download failed")

	var attempts atomic.Int64

	var wg sync.WaitGroup
	for range 10 {

		wg.Go(func() {

			_, err := AcquireModel("op", types.ExecutionProviderCPU, func(types.ExecutionProvider) (any, error) {
				attempts.Add(1)
				return nil, wantErr
			})

			if !errors.Is(err, wantErr) {
				t.Errorf("err = %v, want %v", err, wantErr)
			}
		})
	}

	wg.Wait()

	// Every goroutine either led a build or waited on one. Waiters return the leader's error without retrying, so the
	// attempt count is bounded by the retry limit rather than by the number of callers.
	if got := attempts.Load(); got > maxAcquireAttempts*10 {
		t.Errorf("create attempted %d times, want it bounded", got)
	}

	if got := Registry.Len(); got != 0 {
		t.Errorf("resident = %d after a failed build, want 0", got)
	}
}

// TestFailedBuildDoesNotWedgeTheKey checks that abortBuild really cleared the pending slot: a later acquire of the
// same id must be able to build, not hang or inherit the old error.
func TestFailedBuildDoesNotWedgeTheKey(t *testing.T) {
	withRegistry(t)

	_, err := AcquireModel("op", types.ExecutionProviderCPU, func(types.ExecutionProvider) (any, error) {
		return nil, errors.New("transient")
	})
	if err == nil {
		t.Fatal("expected the first acquire to fail")
	}

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("second acquire after a failed build: %v", err)
	}

	defer lease.Release()

	if got := built.Load(); got != 1 {
		t.Errorf("create called %d times, want 1", got)
	}
}

// TestInUseEntryOutlivesDrain is the issue-34 guarantee: draining the registry while a model is leased must not
// destroy it. The model is destroyed by the last Release instead.
func TestInUseEntryOutlivesDrain(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	model := lease.Model().(*fakeModel)

	DestroyEntries(Registry.DrainAll())

	if model.destroyed.Load() {
		t.Fatal("a leased model was destroyed by DrainAll")
	}

	// Still usable, which is the whole point.
	model.run()

	if got := Registry.Len(); got != 0 {
		t.Errorf("resident = %d after drain, want 0", got)
	}

	lease.Release()

	if !model.destroyed.Load() {
		t.Error("the last Release on a drained entry did not destroy the model")
	}
}

// TestDrainRebuildsRatherThanResurrecting covers locking rule 4: an evicted-but-still-leased entry must be
// unreachable, so a concurrent acquire builds a fresh instance instead of finding the dying one.
func TestDrainRebuildsRatherThanResurrecting(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	held, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	defer held.Release()

	DestroyEntries(Registry.DrainAll())

	fresh, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire after drain: %v", err)
	}

	defer fresh.Release()

	if got := built.Load(); got != 2 {
		t.Errorf("create called %d times, want 2 (the drained one must not be reused)", got)
	}

	if held.Model() == fresh.Model() {
		t.Error("acquire resurrected the drained entry instead of building a fresh one")
	}
}

// TestDoubleReleaseIsNoop guards the idempotence that makes a stray double-defer harmless.
func TestDoubleReleaseIsNoop(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	lease.Release()
	lease.Release()
	lease.Release()

	if got := Registry.Leases(); got != 0 {
		t.Errorf("leases = %d after repeated Release, want 0", got)
	}
}

// TestResidentBytesCharging checks that a Measurable model is charged what it reports, and that a model which can't
// measure itself is charged the conservative default rather than nothing.
func TestResidentBytesCharging(t *testing.T) {
	type unmeasurable struct{}

	tests := []struct {
		name  string
		model any
		want  int64
	}{
		{"measurable model is charged what it reports", &fakeModel{bytes: 4096}, 4096},
		{"a zero measurement means unknown, not free", &fakeModel{bytes: 0}, defaultResidentBytes},
		{"an unmeasurable model is charged the default", &unmeasurable{}, defaultResidentBytes},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := residentBytes(tt.model); got != tt.want {
				t.Errorf("residentBytes() = %d, want %d", got, tt.want)
			}
		})
	}
}

// TestWaitDrainedReturnsImmediatelyWhenIdle covers the common case: nothing was leased, so DrainAll destroyed
// everything itself and there is nothing left to wait for.
func TestWaitDrainedReturnsImmediatelyWhenIdle(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	lease.Release()

	DestroyEntries(Registry.DrainAll())

	if !Registry.WaitDrained(time.Second) {
		t.Error("WaitDrained reported a timeout with nothing outstanding")
	}
}

// TestWaitDrainedBlocksUntilReleased is the guarantee cmd/perf depends on: when the drain returns, the sessions really
// are gone, so the next run measures a genuine cold start.
func TestWaitDrainedBlocksUntilReleased(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	model := lease.Model().(*fakeModel)

	DestroyEntries(Registry.DrainAll())

	// The model is still leased, so the teardown cannot have finished.
	if Registry.WaitDrained(10 * time.Millisecond) {
		t.Fatal("WaitDrained reported success while a model was still leased")
	}

	if model.destroyed.Load() {
		t.Fatal("a leased model was destroyed")
	}

	released := make(chan struct{})

	go func() {
		defer close(released)
		time.Sleep(20 * time.Millisecond)
		lease.Release()
	}()

	if !Registry.WaitDrained(5 * time.Second) {
		t.Error("WaitDrained timed out after the lease was released")
	}

	<-released

	if !model.destroyed.Load() {
		t.Error("the model was not destroyed once its last lease was released")
	}
}

// TestCloseRefusesFurtherAcquisitions checks the shutdown gate: once Close has run, a late request fails as an
// ordinary error rather than building a model into a registry nothing will ever drain again.
func TestCloseRefusesFurtherAcquisitions(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	lease.Release()

	if !Registry.Close(time.Second) {
		t.Error("Close reported a timeout with nothing leased")
	}

	if _, err = acquireFake("op", &built, 10); !errors.Is(err, ErrRegistryClosed) {
		t.Errorf("acquire after Close: err = %v, want ErrRegistryClosed", err)
	}

	if got := built.Load(); got != 1 {
		t.Errorf("create called %d times, want 1 (the post-Close acquire must not build)", got)
	}
}

// TestCloseWaitsForInFlightWork is what makes the ONNX teardown at shutdown safe: Close must report failure rather
// than let Destroy tear the environment down under a live run.
func TestCloseWaitsForInFlightWork(t *testing.T) {
	withRegistry(t)

	var built atomic.Int64

	lease, err := acquireFake("op", &built, 10)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	if Registry.Close(10 * time.Millisecond) {
		t.Error("Close reported success while a model was still in use")
	}

	lease.Release()

	if !Registry.WaitDrained(time.Second) {
		t.Error("the registry never drained after the lease was released")
	}
}

// acquireIn acquires id on a provider that lands in the given pool, so a test can fill one pool without touching the
// other. CUDA maps to the device pool, CPU to the host pool.
func acquireIn(pool types.MemoryPool, id string, bytes int64) (*Lease, error) {
	ep := types.ExecutionProviderCPU
	if pool == types.MemoryPoolDevice {
		ep = types.ExecutionProviderCUDA
	}

	return AcquireModel(id, ep, func(types.ExecutionProvider) (any, error) {
		return &fakeModel{bytes: bytes}, nil
	})
}

// TestBudgetEvictsLeastRecentlyUsed checks the core admission rule: when a pool is full, the model that has gone
// unused the longest makes way, and the ones still in use never do.
func TestBudgetEvictsLeastRecentlyUsed(t *testing.T) {
	withRegistry(t)
	Registry.SetBudget(types.MemoryPoolHost, 250)

	// Three models of 100 bytes each: the third cannot be admitted without evicting one of the first two.
	for _, id := range []string{"first", "second"} {
		lease, err := acquireIn(types.MemoryPoolHost, id, 100)
		if err != nil {
			t.Fatalf("acquire %s: %v", id, err)
		}

		lease.Release()
		// Release stamps lastUsed, so this ordering makes "first" the least recently used.
		time.Sleep(2 * time.Millisecond)
	}

	if got := Registry.Stats().Host.Resident; got != 200 {
		t.Fatalf("resident = %d before the third model, want 200", got)
	}

	third, err := acquireIn(types.MemoryPoolHost, "third", 100)
	if err != nil {
		t.Fatalf("acquire third: %v", err)
	}

	defer third.Release()

	stats := Registry.Stats().Host
	if stats.Resident != 200 {
		t.Errorf("resident = %d, want 200 (one model evicted to make room)", stats.Resident)
	}

	if stats.Models != 2 {
		t.Errorf("models = %d, want 2", stats.Models)
	}

	Registry.mu.Lock()
	_, survived := Registry.entries[registryKey("first", types.ExecutionProviderCPU)]
	Registry.mu.Unlock()

	if survived {
		t.Error("the least recently used model survived; a newer one must have been evicted instead")
	}
}

// TestEntriesAreKeyedByProvider is what makes a change of processor an ordinary cache miss instead of a stop-the-world
// drain: the same operation on two providers is two entries, and neither displaces the other.
func TestEntriesAreKeyedByProvider(t *testing.T) {
	withRegistry(t)

	onCPU, err := acquireIn(types.MemoryPoolHost, "op", 100)
	if err != nil {
		t.Fatalf("acquire on CPU: %v", err)
	}

	defer onCPU.Release()

	onGPU, err := acquireIn(types.MemoryPoolDevice, "op", 100)
	if err != nil {
		t.Fatalf("acquire on CUDA: %v", err)
	}

	defer onGPU.Release()

	if onCPU.Model() == onGPU.Model() {
		t.Error("the same model was reused across two providers; each provider needs its own session")
	}

	stats := Registry.Stats()
	if stats.Host.Models != 1 || stats.Device.Models != 1 {
		t.Errorf("stats = %+v, want one model resident in each pool", stats)
	}
}

// TestFallbackFilesTheModelUnderTheProviderItRanOn covers the re-keying that keeps the registry honest. A CUDA request
// that downgrades must be stored as a CPU model, so that a later explicit CPU request hits it instead of building a
// second identical copy.
func TestFallbackFilesTheModelUnderTheProviderItRanOn(t *testing.T) {
	withRegistry(t)

	t.Cleanup(ResetFallback)
	ResetFallback()

	var built atomic.Int64

	// Fail on anything but the CPU, the way a broken driver does.
	create := func(ep types.ExecutionProvider) (any, error) {
		built.Add(1)

		if ep != types.ExecutionProviderCPU {
			return nil, errors.Join(ErrCreateSession, errors.New("no usable driver"))
		}

		return &fakeModel{bytes: 100}, nil
	}

	lease, err := AcquireModel("op", types.ExecutionProviderCUDA, create)
	if err != nil {
		t.Fatalf("acquire on CUDA: %v", err)
	}

	lease.Release()

	// Two create calls: the CUDA attempt that failed, then the CPU retry.
	if got := built.Load(); got != 2 {
		t.Fatalf("create called %d times, want 2 (a failed attempt then the CPU retry)", got)
	}

	Registry.mu.Lock()
	_, onCPU := Registry.entries[registryKey("op", types.ExecutionProviderCPU)]
	_, onCUDA := Registry.entries[registryKey("op", types.ExecutionProviderCUDA)]
	Registry.mu.Unlock()

	if !onCPU {
		t.Error("the downgraded model was not filed under the CPU")
	}

	if onCUDA {
		t.Error("the downgraded model was filed under CUDA, which is not where it ran")
	}

	// An explicit CPU request must now hit that entry rather than build another copy of it.
	again, err := AcquireModel("op", types.ExecutionProviderCPU, create)
	if err != nil {
		t.Fatalf("acquire on CPU: %v", err)
	}

	defer again.Release()

	if got := built.Load(); got != 2 {
		t.Errorf("create called %d times, want 2 (the CPU request must reuse the downgraded model)", got)
	}

	// And the model must be charged to the host pool, not to the device it was requested on.
	stats := Registry.Stats()
	if stats.Device.Resident != 0 || stats.Host.Resident != 100 {
		t.Errorf("stats = %+v, want the model charged to the host pool", stats)
	}
}

// TestBudgetNeverEvictsAModelInUse is the safety half of admission: pressure must not reclaim something being used.
func TestBudgetNeverEvictsAModelInUse(t *testing.T) {
	withRegistry(t)
	Registry.SetBudget(types.MemoryPoolHost, 150)

	held, err := acquireIn(types.MemoryPoolHost, "held", 100)
	if err != nil {
		t.Fatalf("acquire held: %v", err)
	}

	defer held.Release()

	model := held.Model().(*fakeModel)

	// Admitting this exceeds the budget, but the only candidate is leased, so nothing can be evicted.
	next, err := acquireIn(types.MemoryPoolHost, "next", 100)
	if err != nil {
		t.Fatalf("acquire next: %v", err)
	}

	defer next.Release()

	if model.destroyed.Load() {
		t.Error("a leased model was evicted under budget pressure")
	}

	model.run()
}

// TestOversizedModelIsAdmittedAnyway covers the deliberate escape hatch: a model bigger than the whole budget must
// still load, after everything idle has been evicted. Refusing it would just break the feature it belongs to.
func TestOversizedModelIsAdmittedAnyway(t *testing.T) {
	withRegistry(t)
	Registry.SetBudget(types.MemoryPoolHost, 500)

	small, err := acquireIn(types.MemoryPoolHost, "small", 100)
	if err != nil {
		t.Fatalf("acquire small: %v", err)
	}

	small.Release()

	huge, err := acquireIn(types.MemoryPoolHost, "huge", 5000)
	if err != nil {
		t.Fatalf("an oversized model must still be admitted: %v", err)
	}

	defer huge.Release()

	stats := Registry.Stats().Host
	if stats.Models != 1 || stats.Resident != 5000 {
		t.Errorf("stats = %+v, want the oversized model alone", stats)
	}
}

// TestPoolsAreIsolated checks that filling one pool leaves the other alone - they are not competing for the same
// bytes, and charging a GPU-resident model against system RAM is what the split exists to prevent.
func TestPoolsAreIsolated(t *testing.T) {
	withRegistry(t)
	Registry.SetBudget(types.MemoryPoolHost, 150)
	Registry.SetBudget(types.MemoryPoolDevice, 10_000)

	device, err := acquireIn(types.MemoryPoolDevice, "on-gpu", 100)
	if err != nil {
		t.Fatalf("acquire device model: %v", err)
	}

	device.Release()

	// Fill the host pool hard enough to force evictions there.
	for _, id := range []string{"host-a", "host-b", "host-c"} {
		lease, hostErr := acquireIn(types.MemoryPoolHost, id, 100)
		if hostErr != nil {
			t.Fatalf("acquire %s: %v", id, hostErr)
		}

		lease.Release()
	}

	stats := Registry.Stats()
	if stats.Device.Models != 1 || stats.Device.Resident != 100 {
		t.Errorf("device pool = %+v, want the GPU model untouched by host pressure", stats.Device)
	}

	if stats.Host.Resident > 150 {
		t.Errorf("host resident = %d, want it kept under the 150 budget", stats.Host.Resident)
	}
}

// TestIdleSweepReclaimsUnusedModels covers the TTL half of residency.
func TestIdleSweepReclaimsUnusedModels(t *testing.T) {
	withRegistry(t)
	Registry.SetIdleTTL(time.Millisecond)

	lease, err := acquireIn(types.MemoryPoolHost, "idle", 100)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	model := lease.Model().(*fakeModel)

	// Still in use, so the sweep must leave it alone however idle it looks.
	time.Sleep(5 * time.Millisecond)

	if victims := Registry.sweepIdle(); len(victims) != 0 {
		t.Fatalf("swept %d models while one was leased, want 0", len(victims))
	}

	lease.Release()
	time.Sleep(5 * time.Millisecond)

	DestroyEntries(Registry.sweepIdle())

	if !model.destroyed.Load() {
		t.Error("an idle model was not reclaimed by the sweep")
	}

	if got := Registry.Stats().Host.Resident; got != 0 {
		t.Errorf("resident = %d after the sweep, want 0", got)
	}
}

// TestIdleSweepDisabled checks that a zero TTL means "never sweep", leaving the budget as the only evictor.
func TestIdleSweepDisabled(t *testing.T) {
	withRegistry(t)
	Registry.SetIdleTTL(0)

	lease, err := acquireIn(types.MemoryPoolHost, "kept", 100)
	if err != nil {
		t.Fatalf("acquire: %v", err)
	}

	lease.Release()
	time.Sleep(5 * time.Millisecond)

	if victims := Registry.sweepIdle(); len(victims) != 0 {
		t.Errorf("swept %d models with the TTL disabled, want 0", len(victims))
	}
}

// TestResidentAccountingReturnsToZero guards the counter arithmetic across every path that moves bytes. A leak here
// silently shrinks the usable budget until eviction stops working, with nothing to show for it.
func TestResidentAccountingReturnsToZero(t *testing.T) {
	withRegistry(t)
	Registry.SetBudget(types.MemoryPoolHost, 250)

	for _, id := range []string{"a", "b", "c", "d"} {
		lease, err := acquireIn(types.MemoryPoolHost, id, 100)
		if err != nil {
			t.Fatalf("acquire %s: %v", id, err)
		}

		lease.Release()
	}

	// A failed build must give its reservation back too.
	_, err := AcquireModel("boom", types.ExecutionProviderCPU, func(types.ExecutionProvider) (any, error) {
		return nil, errors.New("nope")
	})
	if err == nil {
		t.Fatal("expected the build to fail")
	}

	DestroyEntries(Registry.DrainAll())

	stats := Registry.Stats()
	if stats.Host.Resident != 0 || stats.Device.Resident != 0 {
		t.Errorf("resident = %+v after draining everything, want 0 in both pools", stats)
	}

	Registry.mu.Lock()
	reserved := Registry.reserved
	Registry.mu.Unlock()

	if reserved[types.MemoryPoolHost] != 0 || reserved[types.MemoryPoolDevice] != 0 {
		t.Errorf("reserved = %v after every build settled, want 0 in both pools", reserved)
	}
}

// TestStressAcquireAgainstDrain is the high-value one.
//
// It reproduces the shape of https://github.com/vegidio/open-photo-ai/issues/34 in pure Go: many goroutines using
// models while another repeatedly tears the registry down underneath them. In production that race ends the process
// from native code, with no panic to catch and nothing useful in the log; here fakeModel turns both halves of it
// (destroying twice, using after destroy) into an immediate panic, and -race covers the bookkeeping.
//
// Run it with: go test -race -run TestStressAcquireAgainstDrain ./internal/
func TestStressAcquireAgainstDrain(t *testing.T) {
	withRegistry(t)

	const (
		workers    = 16
		keys       = 8
		iterations = 200
	)

	var (
		workersDone sync.WaitGroup
		drainerDone sync.WaitGroup
		built       atomic.Int64
		stop        = make(chan struct{})
	)

	// The drainer: destroys everything it can, as fast as it can, until the workers are finished. It must outlive
	// them, or the workers spend most of the run uncontended and the test proves nothing.

	drainerDone.Go(func() {

		for {
			select {
			case <-stop:
				return
			default:
				DestroyEntries(Registry.DrainAll())

				// Drain repeatedly but not in a tight spin. A spin starves waiters by construction - it can always
				// remove an entry between a waiter waking and re-looking - which tests the retry bound rather than
				// the locking. Nothing in the app drains in a loop at all; this is already far past the worst real
				// case of a user hammering the processor switch.
				time.Sleep(50 * time.Microsecond)
			}
		}
	})

	for w := range workers {

		workersDone.Go(func() {

			for i := range iterations {
				// Deterministic key spread, so the test needs no randomness to have workers collide on the same model.
				id := fmt.Sprintf("op-%d", (w+i)%keys)

				lease, err := AcquireModel(id, types.ExecutionProviderCPU, func(types.ExecutionProvider) (any, error) {
					built.Add(1)
					return &fakeModel{bytes: 10}, nil
				})

				if err != nil {
					t.Errorf("acquire %s: %v", id, err)
					return
				}

				// The model must still be alive for as long as the lease is held, no matter what the drainer does.
				lease.Model().(*fakeModel).run()
				lease.Release()
			}
		})
	}

	workersDone.Wait()
	close(stop)
	drainerDone.Wait()

	DestroyEntries(Registry.DrainAll())

	if got := Registry.Leases(); got != 0 {
		t.Errorf("leases = %d after the stress run, want 0", got)
	}

	if got := Registry.Len(); got != 0 {
		t.Errorf("resident = %d after the final drain, want 0", got)
	}

	// Guards the test itself. If the drainer stopped landing - a scheduling change, a future refactor that makes
	// DrainAll a no-op - the workers would sail through uncontended and this test would keep passing while proving
	// nothing. Every rebuild past the first `keys` is a model the drainer successfully destroyed mid-run.
	if got := built.Load(); got <= keys {
		t.Errorf("models built = %d, want well over %d; the drainer never contended", got, keys)
	}
}

// TestInstallAdoptsIncumbent covers the install race directly: the model that lost must be handed back for the caller
// to destroy, and the winner must be the one everyone leases.
func TestInstallAdoptsIncumbent(t *testing.T) {
	withRegistry(t)

	incumbent := &fakeModel{bytes: 10}
	loser := &fakeModel{bytes: 10}

	first, dup, _ := Registry.install("op", "op", types.ExecutionProviderCPU, types.MemoryPoolHost, incumbent, 10, 0)
	if dup != nil {
		t.Fatal("the first install should not report a duplicate")
	}

	second, dup, _ := Registry.install("op", "op", types.ExecutionProviderCPU, types.MemoryPoolHost, loser, 10, 0)
	if dup == nil {
		t.Fatal("the second install should have reported the loser as a duplicate")
	}

	if dup != any(loser) {
		t.Error("install handed back the wrong model as the duplicate")
	}

	if second.Model() != any(incumbent) {
		t.Error("the second lease should point at the incumbent, not the loser")
	}

	first.Release()
	second.Release()
}

// TestInstallTrimsWhenTheModelOutgrewItsEstimate covers the post-install eviction: a model whose real size turns out
// larger than the manifest estimate pushes its pool over budget once it is charged, and install has to hand the
// neighbours it displaced back to the caller.
//
// The trim slice is the only route out for those entries - they are already removed from the map, so nothing else will
// ever find them. Losing it destroys nothing and leaks a live ONNX session per over-estimate.
func TestInstallTrimsWhenTheModelOutgrewItsEstimate(t *testing.T) {
	withRegistry(t)
	Registry.SetBudget(types.MemoryPoolHost, 100)

	// An idle neighbour, installed and released so it is eligible for eviction.
	idle := &fakeModel{bytes: 60}
	lease, _, trim := Registry.install("idle", "idle", types.ExecutionProviderCPU, types.MemoryPoolHost, idle, 60, 0)

	if len(trim) != 0 {
		t.Fatalf("the first install fits the budget and should trim nothing, got %d", len(trim))
	}

	lease.Release()

	// The manifest said this model was free, so nothing was reserved for it; charging its real 60 bytes takes the pool
	// to 120 against a ceiling of 100.
	big := &fakeModel{bytes: 60}
	bigLease, dup, trim := Registry.install("big", "big", types.ExecutionProviderCPU, types.MemoryPoolHost, big, 60, 0)

	if dup != nil {
		t.Fatal("install reported a duplicate where there was no race")
	}

	if len(trim) != 1 || trim[0].model != any(idle) {
		t.Fatalf("install should have trimmed the idle neighbour, got %d entries", len(trim))
	}

	// The trimmed entry is out of the registry and uncharged, leaving only the model that displaced it.
	if got := Registry.Len(); got != 1 {
		t.Errorf("registry holds %d models, want 1", got)
	}

	if got := Registry.Stats().Host.Resident; got != 60 {
		t.Errorf("resident host bytes = %d, want 60", got)
	}

	DestroyEntries(trim)
	bigLease.Release()
}
