package internal

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/types"
)

// numPools is the number of types.MemoryPool values, so the per-pool counters can be plain arrays indexed by the pool.
const numPools = 2

// DefaultIdleTTL is how long an unused model stays resident before the janitor reclaims it. Long enough that a user
// comparing models, or a batch export between images, never pays a rebuild; short enough that walking away from the
// app gives the memory back.
const DefaultIdleTTL = 5 * time.Minute

// janitorInterval is how often idle models are swept for. It is much shorter than DefaultIdleTTL so that the actual
// eviction time isn't quantised to the TTL.
const janitorInterval = 30 * time.Second

// leaseLeakAfter is how long a model may be continuously held before the janitor starts reporting it. Nothing in the
// app legitimately holds one this long, so it is the observable signature of a lease that was never released - the one
// new bug class the refcounting introduces.
const leaseLeakAfter = 15 * time.Minute

// entry is one resident model.
//
// key/id/ep/model/bytes are immutable after construction; refs, lastUsed and evicted are guarded by ModelRegistry.mu.
type entry struct {
	key   string
	id    string                  // operation id, for logging
	ep    types.ExecutionProvider // provider the model was actually built on
	pool  types.MemoryPool        // which budget this entry is charged to, derived from ep at install
	model any
	bytes int64 // measured resident model-file bytes

	refs     int       // outstanding leases; > 0 means "in use, never destroy"
	lastUsed time.Time // stamped on release, not acquire, so a long batch never looks idle mid-run
	evicted  bool      // removed from the map; the last Release destroys it
}

// Lease is a live borrow of a model, keeping it alive until released.
//
// Release must be called exactly once - use a deferred call placed immediately after the error check - and the model
// must not be touched afterwards. Never copy a Lease; always pass *Lease.
type Lease struct {
	reg      *ModelRegistry
	e        *entry
	released atomic.Bool
}

// Model returns the leased model. It must not be used after Release.
func (l *Lease) Model() any {
	return l.e.model
}

// Release returns the lease. It is idempotent, so a stray double-defer is harmless, and it destroys the model when it
// is the last lease on an entry that has already been removed from the registry.
func (l *Lease) Release() {
	if l == nil || l.released.Swap(true) {
		return
	}

	if victim := l.reg.release(l.e); victim != nil {
		destroyEntry(victim)
	}
}

// build is an in-flight model creation. Exactly one goroutine per key is the leader and does the work; the rest wait
// on done and then retry the lookup.
type build struct {
	done chan struct{}
	err  error
}

// ModelRegistry holds the models currently resident in memory, keyed by operation ID.
//
// Liveness is tracked per entry with a refcount rather than with a process-wide lock: a model is only ever destroyed
// when nothing holds a lease on it, so freeing one model never blocks inference on another. Destroying a model
// releases its ONNX session, and doing that while another goroutine is running inference on it is a use-after-free in
// native code that terminates the process rather than panicking (see
// https://github.com/vegidio/open-photo-ai/issues/34) - the refcount is what prevents it.
//
// Locking rules, in order of how badly they break things when violated:
//
//  1. mu is a plain Mutex, never an RWMutex. It is held only for map and counter work - never across create, never
//     across Destroy, never across I/O. Helpers that assume it is already held are named lockedFoo.
//  2. Destroy is never called under mu. Every mutating primitive returns the entries it removed, and the caller
//     destroys them after unlocking.
//  3. Destroy is only ever called when refs == 0.
//  4. An entry present in entries is always usable. Removal happens first, so a concurrent acquire misses and builds a
//     fresh instance instead of resurrecting a dying one. No handshake, no waiting.
type ModelRegistry struct {
	mu      sync.Mutex
	entries map[string]*entry
	pending map[string]*build // in-flight creations, one per key (single-flight)
	leases  int               // outstanding leases across all entries, for diagnostics
	closed  bool              // set by Close; no further models may be acquired

	// resident is the bytes currently charged to each pool; reserved is the bytes promised to builds that are in
	// flight but not yet installed. Admission tests resident+reserved+want, so two concurrent large builds cannot both
	// pass a check that only one of them fits through. budget of 0 means the pool is unbounded.
	resident [numPools]int64
	reserved [numPools]int64
	budget   [numPools]int64
	idleTTL  time.Duration

	// janitorStop is both the stop channel and the "is the sweep running?" flag: stopJanitor clears it, which is what
	// lets a registry that was closed and reopened start a fresh janitor instead of being stuck with the dead one.
	janitorStop chan struct{}

	// evicting counts entries that have been removed from the map but are still leased, and so cannot be destroyed
	// until their last user finishes. drained is non-nil exactly while evicting > 0, and is closed when the count
	// reaches zero - that is how a caller waits for a teardown to actually complete rather than merely be requested.
	evicting int
	drained  chan struct{}
}

// ErrRegistryClosed is returned by AcquireModel once the runtime is shutting down. It exists so a request that races
// Destroy fails as an ordinary error instead of building a model into a registry that will never be drained again.
var ErrRegistryClosed = errors.New("model registry is closed")

func newModelRegistry() *ModelRegistry {
	return &ModelRegistry{
		entries: make(map[string]*entry),
		pending: make(map[string]*build),
	}
}

// acquireOrBuild resolves key to one of three outcomes, atomically:
//
//   - the model is resident, and lease is a live borrow of it;
//   - a build is already in flight, and the caller must wait on b.done before retrying;
//   - nothing exists yet, leader is true, and the caller must build it and then resolve b.
//
// The lookup and the build registration have to happen under one acquisition of mu. Splitting them lets a leader
// install its model AND clear the pending slot in the gap between a second caller's failed lookup and its own
// registration, which promotes that caller to leader too and builds a duplicate - the exact leak single-flighting is
// here to prevent.
func (r *ModelRegistry) acquireOrBuild(key string) (lease *Lease, b *build, leader bool, err error) {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.closed {
		return nil, nil, false, ErrRegistryClosed
	}

	if e, ok := r.entries[key]; ok {
		return r.lockedLease(e), nil, false, nil
	}

	if pending, ok := r.pending[key]; ok {
		return nil, pending, false, nil
	}

	pending := &build{done: make(chan struct{})}
	r.pending[key] = pending

	return nil, pending, true, nil
}

// lockedLease hands out a lease on e. Callers must hold mu.
func (r *ModelRegistry) lockedLease(e *entry) *Lease {
	e.refs++
	r.leases++

	return &Lease{reg: r, e: e}
}

// resolveBuild releases the waiters on a finished build. err is nil when it succeeded, and otherwise is handed to the
// waiters so they fail the same way instead of each retrying a creation that just proved impossible.
//
// Success and failure share one function because the invariant is the same for both: the leader must always resolve
// its pending build, or every waiter blocks forever.
func (r *ModelRegistry) resolveBuild(key string, err error) {
	r.mu.Lock()
	b := r.pending[key]
	delete(r.pending, key)
	r.mu.Unlock()

	if b != nil {
		b.err = err
		close(b.done)
	}
}

// makeRoom evicts idle models from pool p, least-recently-used first, until want bytes fit under that pool's budget,
// and reserves want for the caller's in-flight build.
//
// Entries in use are never evicted, and entries in the other pool are never considered - they are not competing for
// the same bytes. The victims are returned for the caller to destroy after unlocking.
//
// Freeing happens before the build rather than after it because the peak is what runs the machine out of memory: a
// 7 GB model that is admitted and then trims its neighbours has already had to allocate 7 GB alongside them.
//
// When evicting every idle model still isn't enough, the build is admitted anyway with a warning. A model larger than
// the whole budget has to remain loadable - refusing it would simply break the feature it belongs to.
func (r *ModelRegistry) makeRoom(p types.MemoryPool, want int64) []*entry {
	r.mu.Lock()

	victims, stillOver := r.lockedEvictUntilFits(p, want)
	r.reserved[p] += want
	resident, budget := r.resident[p], r.budget[p]

	r.mu.Unlock()

	warnOverBudget(p, want, resident, budget, stillOver)

	return victims
}

// warnOverBudget reports a model admitted despite not fitting, which both the pre-build reservation and the post-build
// install have to do. One function so the two cannot drift into describing the same event differently.
func warnOverBudget(p types.MemoryPool, want, resident, budget int64, stillOver bool) {
	if !stillOver {
		return
	}

	Log().Warn("model does not fit the memory budget; admitting it anyway",
		"pool", p, "want", want, "resident", resident, "budget", budget)
}

// lockedEvictUntilFits evicts idle models from pool p, least-recently-used first, until want more bytes fit under that
// pool's ceiling, and reports whether the pool is still over budget once there is nothing left to evict.
//
// An unbounded pool evicts nothing and is never over. Callers must hold mu, and must destroy the victims after
// unlocking.
func (r *ModelRegistry) lockedEvictUntilFits(p types.MemoryPool, want int64) (victims []*entry, stillOver bool) {
	if r.budget[p] <= 0 {
		return nil, false
	}

	for r.lockedOverBudget(p, want) {
		victim := r.lockedOldestIdle(p)
		if victim == nil {
			break
		}

		if evicted := r.lockedEvict(victim); evicted != nil {
			victims = append(victims, evicted)
		}
	}

	return victims, r.lockedOverBudget(p, want)
}

// lockedEvict uncharges e from its pool and removes it from the registry, returning it when it can be destroyed right
// now and nil when a live lease has to finish first.
//
// Uncharging before the removal is the invariant every eviction path shares, which is why they all go through here.
// Callers must hold mu, and must destroy what they collect after unlocking.
func (r *ModelRegistry) lockedEvict(e *entry) *entry {
	r.resident[e.pool] -= e.bytes

	if r.lockedStartEvicting(e) {
		return e
	}

	return nil
}

// lockedOverBudget reports whether admitting want more bytes would exceed pool p's ceiling. Callers must hold mu.
func (r *ModelRegistry) lockedOverBudget(p types.MemoryPool, want int64) bool {
	return r.resident[p]+r.reserved[p]+want > r.budget[p]
}

// lockedOldestIdle returns the least recently used unleased entry in pool p, or nil if there is none.
//
// A linear scan is deliberate: the registry holds well under twenty models, so the bookkeeping a heap or an intrusive
// list would need costs more than it saves. Callers must hold mu.
func (r *ModelRegistry) lockedOldestIdle(p types.MemoryPool) *entry {
	var oldest *entry

	for _, e := range r.entries {
		if e.refs > 0 || e.pool != p {
			continue
		}

		if oldest == nil || e.lastUsed.Before(oldest.lastUsed) {
			oldest = e
		}
	}

	return oldest
}

// releaseReservation gives back bytes promised to a build that never installed.
func (r *ModelRegistry) releaseReservation(p types.MemoryPool, want int64) {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.reserved[p] -= want
}

// install stores a freshly built model under key and returns a lease on it, converting the caller's reservation into
// a real charge against the pool.
//
// If another goroutine won the race and installed an entry under the same key first, the incumbent is leased instead
// and the freshly built model is handed back as dup for the caller to destroy - two live sessions for one key would
// leak whichever one lost.
//
// trim carries any models evicted because the finished model turned out larger than the pre-build estimate, which is
// the normal case for an operation the manifest can't size ahead of time.
func (r *ModelRegistry) install(
	key, id string,
	ep types.ExecutionProvider,
	pool types.MemoryPool,
	model any,
	bytes, reserved int64,
) (lease *Lease, dup any, trim []*entry) {
	r.mu.Lock()

	r.reserved[pool] -= reserved

	// The registry closed while this model was building. Filing it now would put a live session behind a DrainAll that
	// has already walked past it, leaving it to outlive DestroyEnvironment - the exact native use-after-free the
	// refcount exists to prevent. Hand it back as dup so the caller destroys it, same as losing an install race.
	if r.closed {
		r.mu.Unlock()

		return nil, model, nil
	}

	if incumbent, ok := r.entries[key]; ok {
		lease = r.lockedLease(incumbent)
		r.mu.Unlock()

		return lease, model, nil
	}

	e := &entry{
		key:      key,
		id:       id,
		ep:       ep,
		pool:     pool,
		model:    model,
		bytes:    bytes,
		lastUsed: time.Now(),
	}

	r.entries[key] = e
	r.resident[pool] += bytes

	// The new entry is leased before the trim, so it can never evict itself.
	lease = r.lockedLease(e)

	trim, stillOver := r.lockedEvictUntilFits(pool, 0)
	resident, budget := r.resident[pool], r.budget[pool]

	r.mu.Unlock()

	warnOverBudget(pool, bytes, resident, budget, stillOver)

	return lease, nil, trim
}

// release drops one lease on e, returning e when it is now unused AND already removed from the registry, which makes
// the caller responsible for destroying it.
func (r *ModelRegistry) release(e *entry) *entry {
	r.mu.Lock()
	defer r.mu.Unlock()

	e.refs--
	r.leases--
	e.lastUsed = time.Now()

	if e.refs == 0 && e.evicted {
		r.lockedFinishEvicting()
		return e
	}

	return nil
}

// lockedStartEvicting removes e from the map and reports whether it can be destroyed right now.
//
// An entry that is still leased is left to its last Release instead, and counted so a caller waiting for the teardown
// knows it hasn't finished. Callers must hold mu.
func (r *ModelRegistry) lockedStartEvicting(e *entry) (destroyNow bool) {
	delete(r.entries, e.key)
	e.evicted = true

	if e.refs == 0 {
		return true
	}

	r.evicting++
	if r.drained == nil {
		r.drained = make(chan struct{})
	}

	return false
}

// lockedFinishEvicting records that one evicted entry has become destroyable, waking anyone waiting for the teardown
// to complete. Callers must hold mu.
func (r *ModelRegistry) lockedFinishEvicting() {
	r.evicting--

	if r.evicting == 0 && r.drained != nil {
		close(r.drained)
		r.drained = nil
	}
}

// WaitDrained blocks until every entry removed from the registry has actually been destroyed, or timeout elapses. It
// reports whether the teardown completed.
//
// This is what makes a drain synchronous without a process-wide lock: it waits on the models that were removed, rather
// than on all inference everywhere.
func (r *ModelRegistry) WaitDrained(timeout time.Duration) bool {
	r.mu.Lock()
	waitFor := r.drained
	r.mu.Unlock()

	// Nothing was left outstanding, so the teardown finished inside DrainAll.
	if waitFor == nil {
		return true
	}

	select {
	case <-waitFor:
		return true
	case <-time.After(timeout):
		return false
	}
}

// Close stops the registry accepting new acquisitions, destroys everything it can, and waits up to timeout for models
// still in use to be released. It reports whether the teardown completed.
func (r *ModelRegistry) Close(timeout time.Duration) bool {
	r.stopJanitor()

	r.mu.Lock()
	r.closed = true
	r.mu.Unlock()

	DestroyEntries(r.DrainAll())

	return r.WaitDrained(timeout)
}

// DrainAll removes every entry from the registry and returns the ones that are safe to destroy right now.
//
// Entries still in use are marked evicted and left to their last Release, so draining never waits on inference and
// never frees a session out from under a running model. They are already unreachable: rule 4 means the next acquire
// builds a fresh instance rather than finding one of these.
func (r *ModelRegistry) DrainAll() []*entry {
	r.mu.Lock()
	defer r.mu.Unlock()

	victims := make([]*entry, 0, len(r.entries))

	for _, e := range r.entries {
		if evicted := r.lockedEvict(e); evicted != nil {
			victims = append(victims, evicted)
		}
	}

	return victims
}

// SetBudget sets the ceiling for one pool, in bytes. Zero makes the pool unbounded.
//
// Lowering it does not evict anything on its own; the new ceiling applies to the next admission, and the janitor
// reclaims whatever goes idle in the meantime. That keeps a settings change from stalling work already in flight.
func (r *ModelRegistry) SetBudget(p types.MemoryPool, bytes int64) {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.budget[p] = max(bytes, 0)
}

// SetIdleTTL sets how long an unused model stays resident. Zero disables the idle sweep, leaving the budget as the
// only thing that evicts.
func (r *ModelRegistry) SetIdleTTL(ttl time.Duration) {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.idleTTL = max(ttl, 0)
}

// Stats reports what each pool currently holds.
func (r *ModelRegistry) Stats() types.ModelMemory {
	r.mu.Lock()
	defer r.mu.Unlock()

	var counts [numPools]int
	for _, e := range r.entries {
		counts[e.pool]++
	}

	pool := func(p types.MemoryPool) types.PoolMemory {
		return types.PoolMemory{
			Pool:     p,
			Resident: r.resident[p],
			Budget:   r.budget[p],
			Models:   counts[p],
		}
	}

	return types.ModelMemory{
		Device: pool(types.MemoryPoolDevice),
		Host:   pool(types.MemoryPoolHost),
	}
}

// Available reports how many bytes are still under a pool's ceiling: budget - resident - reserved.
//
// It returns 0 for an unbounded pool, which callers must read as "unknown" rather than "nothing free" - and, since it
// is a snapshot of a value other builds are moving, as advice rather than a guarantee. What it measures is also the
// file-size proxy Stats reports, not real occupancy, so a caller sizing a workload against it needs its own margin on
// top. It exists for the diffusion upscaler, which has to choose between one whole-image pass and a tiled one before
// it allocates anything.
func (r *ModelRegistry) Available(p types.MemoryPool) int64 {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.budget[p] <= 0 {
		return 0
	}

	return max(r.budget[p]-r.resident[p]-r.reserved[p], 0)
}

// StartJanitor begins the background sweep that reclaims idle models. It is safe to call more than once; only the
// first call starts a goroutine.
func (r *ModelRegistry) StartJanitor() {
	r.mu.Lock()

	if r.janitorStop != nil {
		r.mu.Unlock()
		return
	}

	stop := make(chan struct{})
	r.janitorStop = stop
	r.mu.Unlock()

	// The goroutine closes over the local rather than reading r.janitorStop, so stopJanitor can clear the field
	// without racing the select below.
	go func() {
		ticker := time.NewTicker(janitorInterval)
		defer ticker.Stop()

		for {
			select {
			case <-stop:
				return
			case <-ticker.C:
				DestroyEntries(r.sweepIdle())
			}
		}
	}()
}

// Reopen lifts the shutdown latch set by Close so the registry accepts models again.
//
// Close is deliberately one-way for the run it ends - a model must never be filed into a registry that has already
// been drained. Reopen exists because Initialize/Destroy is a lifecycle, not a process: an embedder reconfiguring the
// runtime, or a test doing this repeatedly, would otherwise be left with a registry where every acquisition fails
// with ErrRegistryClosed forever.
func (r *ModelRegistry) Reopen() {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.closed = false
}

// stopJanitor ends the background sweep. It is safe to call when the janitor was never started.
func (r *ModelRegistry) stopJanitor() {
	r.mu.Lock()
	stop := r.janitorStop
	r.janitorStop = nil
	r.mu.Unlock()

	if stop != nil {
		close(stop)
	}
}

// sweepIdle removes models that have gone unused for longer than the idle TTL, returning them for the caller to
// destroy after unlocking.
//
// It also reports models that have been held continuously for a very long time. Nothing in the app does that, so it
// almost certainly means a lease was never released - which silently shrinks the budget and stops eviction working,
// and is otherwise invisible.
//
// The log lines are collected under mu and emitted after it is released. The library logger writes to a rotating file,
// so writing one per entry while holding the lock would block every concurrent acquire and release on disk I/O - the
// thing rule 1 exists to prevent.
func (r *ModelRegistry) sweepIdle() []*entry {
	// note is one pending log line: which model, how long it has been that way, and the field that distinguishes the
	// two cases - refs for a suspected leak, bytes for an eviction.
	type note struct {
		id      string
		elapsed time.Duration
		refs    int
		bytes   int64
	}

	var (
		victims []*entry
		evicted []note
		held    []note
	)

	r.mu.Lock()

	if r.idleTTL <= 0 {
		r.mu.Unlock()
		return nil
	}

	now := time.Now()

	for _, e := range r.entries {
		elapsed := now.Sub(e.lastUsed)

		if e.refs > 0 {
			if elapsed > leaseLeakAfter {
				held = append(held, note{id: e.id, elapsed: elapsed, refs: e.refs})
			}

			continue
		}

		if elapsed <= r.idleTTL {
			continue
		}

		evicted = append(evicted, note{id: e.id, elapsed: elapsed, bytes: e.bytes})

		if victim := r.lockedEvict(e); victim != nil {
			victims = append(victims, victim)
		}
	}

	r.mu.Unlock()

	// Warn rather than Debug: a leaked lease pins its model - and its device memory - for the rest of the process,
	// which is a bug in this package, not a routine event. It fires at most once per sweep per model, and at Debug it
	// could only ever be seen by someone who already suspected it.
	for _, n := range held {
		Log().Warn("model has been held for a long time; a lease may have leaked",
			"op", n.id, "refs", n.refs, "held_for", n.elapsed)
	}

	for _, n := range evicted {
		Log().Debug("evicting idle model", "op", n.id, "idle_for", n.elapsed, "bytes", n.bytes)
	}

	return victims
}

// Len reports how many models are resident. For tests and diagnostics.
func (r *ModelRegistry) Len() int {
	r.mu.Lock()
	defer r.mu.Unlock()

	return len(r.entries)
}

// Leases reports how many leases are outstanding. For tests and diagnostics.
func (r *ModelRegistry) Leases() int {
	r.mu.Lock()
	defer r.mu.Unlock()

	return r.leases
}

// Registry is where all loaded models are stored.
//
// This variable is driven by AcquireModel and CleanRegistry and should never be mutated directly.
var Registry = newModelRegistry()

// destroyEntry releases the native resources behind an entry. It must only be called with refs == 0 and without
// holding the registry lock.
func destroyEntry(e *entry) {
	Log().Debug("destroying model", "op", e.id, "ep", e.ep, "bytes", e.bytes)
	destroyModel(e.model)
}

// destroyModel releases the native resources behind a model, if it holds any.
func destroyModel(model any) {
	if destroyable, ok := model.(types.Destroyable); ok {
		destroyable.Destroy()
	}
}

// DestroyEntries destroys a batch of removed entries. Call it after unlocking, never under mu.
func DestroyEntries(entries []*entry) {
	for _, e := range entries {
		destroyEntry(e)
	}
}

// residentBytes reports how much memory a model should be charged, falling back to a conservative default for models
// that can't measure themselves so that forgetting types.Measurable makes the accounting coarser rather than blind.
func residentBytes(model any) int64 {
	measurable, ok := model.(types.Measurable)
	if !ok {
		Log().Warn("model does not report its size; charging the default", "bytes", defaultResidentBytes)
		return defaultResidentBytes
	}

	if bytes := measurable.ResidentBytes(); bytes > 0 {
		return bytes
	}

	// A Measurable that reports 0 means the files behind it couldn't be sized, not that it is free.
	Log().Warn("model reported zero size; charging the default", "bytes", defaultResidentBytes)

	return defaultResidentBytes
}
