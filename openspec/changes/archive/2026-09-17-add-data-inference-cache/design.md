## Context

See proposal.md — Why. What matters for the approach is what is already in place.

`Opai::process` passes the application's store handle down to `chain::process`, which combines it with
`ProcessOptions::cache` **once** into a `RunCache` the whole run holds. `chain::run` then walks the chain
prefix by prefix, and per step:

```
key = identity_after(cache.source(), &operations[..=index], depth)
  |
  +-- get_bytes(key) on a blocking thread   -> hit:  progress.operation(..).cached(); prefix advances, nothing acquired
  |                                            miss: fall through
  +-- pipeline::acquire(..)                     the install + session build
  +-- spawn_blocking { ImagePipeline::run }
  +-- cache.put(key, &result)
```

The hit arm is **above** `pipeline::acquire`, which is the property this change is copying.

`Opai::execute` passes no handle at all. `inference/execute.rs` builds a `ChainProgress::new(on_progress, 1)`, calls
`pipeline::acquire`, hops to a blocking thread and folds a `ProviderReport` out of the handles. There is no store, no
key and no `cache` field in `ExecuteOptions`.

Three things about `cache.rs` decide how small this can be:

- `get_bytes` and the `set_bytes` call with its `NotAdmitted`/broken-write logging are already **value-agnostic** —
  they move `Vec<u8>` and know nothing about pictures.
- `get`, `put` and `decode` are the picture-specific layer on top, and are the only three that are.
- `ENTRY_TTL` is already the 24 hours the requirement asks for, and is already asserted against a literal
  `86_400` so it cannot drift.

Constraints that shape everything below: the 15-task limit, the module-layout doctrine (`inference/` holds only what
no model owns), and the requirement that everything but the model run stay exercisable on a CI runner with no ONNX
Runtime, no GPU, no model file and no network.

## Goals / Non-Goals

**Goals:**

- A repeated non-image run served without transferring, opening or executing anything.
- The caching generic over the operation's result type, so the next data family needs no further change here.
- `chain`'s behaviour **bit-identical**. This change adds a sibling derivation beside `identity_after`; it does not
  touch the chain's own path, and the chain's tests must pass unedited.
- Everything exercisable with no runtime: the key derivation, the encoding round trip, the hit and miss arms, the
  cached progress report, the provider report on a hit, and every degradation path.

**Non-Goals:**

- **No second store and no new backing mode.** This is the `Memo` the application already opens.
- **No coalescing of concurrent identical runs.** `cache.rs` already records why the chain declined it, and the
  argument is unchanged here: it would make one caller's cancellation fail an unrelated run.
- **No front-end work.** Neither `gui` nor `cli` calls `execute`.
- **No change to the enhancement path**, its keys, or what it stores.
- **No size bound on the store.** Unbounded-until-sweep is the store's existing documented behaviour and belongs in
  `memo` if it is to change.

## Decisions

### D1. The result type carries a serialization bound on the sealed trait

```rust
pub(crate) mod sealed {
    pub trait DataModel {
        type Output: serde::Serialize + serde::de::DeserializeOwned + Send + 'static;
        fn as_operation(&self) -> Operation;
        fn pipeline<B: Backend>(&self) -> Result<SharedData<B, Self::Output>, InferenceError>;
    }
}
```

The bound goes on the **associated type**, not on `execute`'s signature. Putting it on the function would let an
operation exist that cannot be cached and be discovered only at the call site; putting it on the type makes "a
non-image result is storable" a property every implementor proves once, which is what the requirement's "every
operation on this path" means.

`Send + 'static` moves here from `execute`'s `where O::Output: Send + 'static` clause, which then disappears from
four signatures. That is a simplification the bound pays for rather than a separate change.

**Cost: none today.** `Faces`, `Face`, `Point`, `Rect` and `Confidence` already derive both halves, so `Detection`
satisfies it unchanged. The trait is sealed and this crate is its only implementor, so the narrowing cannot break an
external caller.

**Alternative rejected:** a separate `CacheableOperation` trait that `execute` takes when the caller wants caching.
It doubles the trait surface to express "some data operations are not cacheable" — a case that does not exist and
that the spec forbids, since caching is required of the path rather than offered by it.

### D2. JSON, and why not a compact binary encoding

`serde_json` is already a dependency and needs no feature work. The alternative worth naming is a binary format
(`bincode`, `postcard`, `rmp-serde`), which would be perhaps three times smaller and faster.

It is not taken, for two reasons. The first is `rust-sak`: the store is `memo`'s, this project adds encodings on top
of it, and adding a third-party crate to serialise a 200-byte value is the kind of dependency the project's rules
ask to justify rather than assume. The second is that the size argument does not survive contact with the numbers —
a detection of a crowded photograph is a few dozen faces, so a stored entry is kilobytes against the *megabytes* an
enhanced picture entry holds. Three times a rounding error is a rounding error.

If a later data family produces something large — a per-pixel mask, say — that is the change that should
re-open this, with a measurement rather than a preference.

### D3. The two kinds of entry cannot be read as one another, twice over

The requirement is that a picture entry is never decoded as a result or the reverse. This holds by two independent
mechanisms, and both are kept deliberately:

- **The keys are disjoint by construction.** See D4 — a data key folds in a discriminator no chain key can produce.
  Two kinds of entry therefore never collide on a key in the first place.
- **The encodings reject each other.** JSON bytes are not a valid PNG and PNG bytes are not valid JSON, so a read of
  the wrong kind is an `Err` that folds into a miss — which is a re-run, the same response every other store failure
  gets.

Relying on the keys alone would make the guarantee depend on a string derivation staying correct forever; relying on
the encodings alone would make it depend on two formats never overlapping. Each covers the other's failure, and
neither costs anything to keep.

### D4. A data key is derived beside `identity_after`, not by it

```rust
// inference/chain.rs, beside identity_after
pub(crate) fn identity_of_data(identity: &str, operation: &Operation) -> String
```

It folds `identity`, the operation's cache tag and a fixed discriminator that is **not** a `ChannelDepth::tag()`.
Depth tags are `"8"` and `"16"`; the discriminator is a word, so no chain key and no data key can ever be the same
string however the inputs vary.

**It lives beside `identity_after` rather than in `execute.rs`**, because the property that matters is a relationship
*between* the two derivations, and a property of a pair is not checkable from inside one of them. A test asserting
they never collide has to see both, and a later edit to either is then visibly an edit to a pair.

**One derivation, not two.** As on the chain path, the read and the write compute the same value from the same
function, so they cannot disagree about a key neither of them owns.

**The provider is not in the key**, matching both the requirement and `cache.rs`'s existing argument for the chain:
varying on it would halve the hit rate on a machine whose GPU is sometimes busy, and a caller who needs each provider
to actually run turns the store off instead.

### D5. A value pair beside the picture pair in `cache.rs`

```rust
impl RunCache {
    pub(crate) fn get_value<T: DeserializeOwned>(&self, key: &str) -> Option<T>;
    pub(crate) fn put_value<T: Serialize>(&self, key: &str, value: &T);
}
```

Written over the existing `get_bytes` and the existing `set_bytes` arm, so the entry lifetime, the declined-write
`debug`, the broken-write `warn` and the `source`-carrying records are **reused rather than repeated**. What is new
is two `serde_json` calls and a `warn` for a value that would not decode — the exact shape `decode` already has for
a picture that would not.

They are `RunCache`'s methods rather than free functions because every one of the store's failure rules is already
written on that type, and a second entry point that had to re-derive "a failure costs a re-run and nothing else"
would be the place the rule eventually diverges.

### D6. The driver gains the chain's hit arm, in the chain's order

`inference/execute.rs`, with the lookup above the acquisition:

1. Resolve the store **once** from the handle and `ExecuteOptions::cache`, as `chain::process` does — so a run
   cannot read from the store and then decline to write to it.
2. Derive the key with D4.
3. `get_value` on a blocking thread. On a hit: `progress.operation(0, ..).cached()`, and return with a
   `ProviderReport::new(provider, [])`.
4. On a miss: the existing path unchanged — `pipeline::acquire`, the blocking hop, the handles held until return.
5. `put_value` after a successful run, on the blocking thread that already exists.

**The hit returns `ProviderReport::new(provider, std::iter::empty())`**, which is the same fold the miss path uses
over an empty set of handles. `ProviderVerdict::NothingExecuted` then falls out of `ProviderReport`'s own logic
rather than being a second place that decides what a cached run reports — the requirement is satisfied by reusing
the rule, not by restating it.

**`.cached()` reports `Stage::Cached` at `1.0`** through the same clamped path every other report uses, which is why
a served run still ends on exactly 1 rather than never reporting.

**Cancellation is still checked before the lookup.** A caller who stopped the run must not be handed a stored result
as though the run had completed.

**Both arms record which way the run went**, at `debug` and naming the operation and the slot, which is the chain's
own pair rather than a second scheme. `logging` requires it of an enhancement's steps because those were every read
of the store there was; a second reader of the same store inherits the requirement rather than escaping it, and the
requirement is broadened to say so. Naming the slot on both is what lets a miss and the hit it later serves be read
as one sequence — without it, "the store is caching nothing", which looks exactly like a store that is working, is
not answerable from the file.

### D7. Ownership and lifetimes

Unchanged on the miss path, and that is the point: the driver owns every `SessionHandle` for the whole call and
drops them on return, the handles travel into the blocking closure and back out, and the session cache's "in use"
test is the handle's own reference count.

**A hit acquires no handle at all**, so there is nothing to hold and nothing that could be reclaimed mid-run. It
also means a hit cannot keep a model resident: a run served from the store does not touch the session cache, so it
neither extends a model's life nor prevents the idle sweep from reclaiming one. That is correct — a run that did no
inference has no claim on a session — and is worth stating because it is the one ownership difference between the
two arms.

The `RunCache` is cloned into the blocking closure, as the chain clones it. It is a `Memo` handle plus a `String`;
`memo` owns the concurrency behind it.

### D8. Crates and dependencies

**`opai` only.** `gui` and `cli` are untouched, and nothing here is logic a front end would need — it sits under an
entry point neither calls yet.

**`rust-sak`: nothing new, and nothing missing.** `memo` supplies the store, its three backing modes, the per-entry
lifetime and the sweep; `crypto`'s `xxh3_string` supplies the key hashing. Both are already enabled and already used
by `cache.rs` for exactly this. Nothing here is a candidate for extending `rust-sak`: the encoding is of this
project's own result types and the key derivation is over this project's own operations, neither of which is
general-purpose.

**No new third-party crate.** `serde` and `serde_json` are already dependencies — see D2 for why a binary format is
not taken.

### D9. Platforms and providers this design is validated against

- **All of it runs on every CI platform with no ONNX Runtime, no GPU, no model file and no network.** The key
  derivation, the encoding round trip, both arms, the cached progress report, the provider report on a hit and every
  degradation path are arithmetic and plumbing over the `Backend` seam and a memory-backed `Memo`. This change adds
  nothing that needs a runtime.
- **The real path is exercised on macOS, on the CPU and CoreML providers**, which is the development machine.
- **CUDA and TensorRT are untested here**, as for every change so far. Nothing in this design is provider-dependent:
  the provider is deliberately not part of a key.
- **Windows and Linux are untested beyond CI's arithmetic.** The one platform-sensitive piece is the disk-backed
  store, which is `memo`'s and is already exercised by the enhancement path on all three.

## Risks / Trade-offs

**A stale entry outlives a change to the model or the decode** → A result stored today is served tomorrow even if
the detector's arithmetic changed in between, because the key folds the operation's cache tag and not the code that
implements it. This is the enhancement path's existing exposure, not a new one — the same is true of every stored
picture — and the mitigation is the same: the tag changes when the model does, and the 24-hour lifetime bounds
anything that slips through. Stated rather than fixed, because fixing it means keying on a build identifier and
throwing the store away on every rebuild.

**The two key derivations could converge under a later edit** → A chain key and a data key that collided would serve
a picture where a result belongs, which D3's second mechanism turns into a miss rather than a wrong answer, but
which would silently cost every affected run its cache. Mitigated as a verification rather than as care: a test
drives both derivations over the same identity and operation and asserts they differ, so an edit to either that
removed the discriminator fails rather than degrades.

**A serialization bound on a published trait narrows it** → `DataOperation` is sealed and this crate is its only
implementor, so nothing external can be broken today. The cost is borne by a future result type that is expensive or
awkward to serialise, which would then have to serialise anyway or change this bound. Accepted: the spec requires
caching of this path, so a result that cannot be stored is a result that cannot satisfy the requirement.

**A hit no longer keeps a model resident** → A caller that ran detection repeatedly to keep the detector warm loses
that side effect. No such caller exists — nothing calls `execute` yet — and relying on a cache miss to manage session
residency would be the wrong mechanism anyway; the session cache's idle sweep is where that belongs.

## Open Questions

- **Whether a store entry should carry a version tag alongside the value.** It would let a later change invalidate
  every stored result of one kind without waiting out the lifetime or clearing the whole store. Nothing in this
  change needs it, the enhancement path does not have it either, and adding it later is an encoding change behind
  `get_value`/`put_value` that nothing above those two functions would see. Deferred deliberately rather than
  skipped: it changes no requirement, no approach and no task here.
