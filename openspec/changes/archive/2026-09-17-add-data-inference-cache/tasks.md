## 1. The result type becomes storable

- [x] 1.1 Put design.md D1's bound on `sealed::DataModel::Output` in `models/operation.rs` —
      `Serialize + DeserializeOwned + Send + 'static` — and delete the now-redundant `where O::Output: Send + 'static`
      clause from `Opai::execute`, `inference::execute::execute`, its inner `run` and anywhere else it is spelled.
      Document on the associated type *why* the bound is there: the path's results are kept, so being storable is a
      property an implementing type proves once rather than a call site discovers. Verify `cargo build -p opai
      --all-targets` is clean with **no change to `Detection`'s impl** — `Faces` already derives both halves, and a
      derive that had to be added means the bound landed on the wrong type — and that the `compile_fail` doctest
      pinning the seal still passes.

## 2. The store learns a second kind of value

- [x] 2.1 Add `RunCache::get_value<T: DeserializeOwned>` and `RunCache::put_value<T: Serialize>` to `cache.rs` per
      design.md D5, written over the **existing** `get_bytes` and the existing `set_bytes` arm so the entry lifetime,
      the `NotAdmitted` `debug`, the broken-write `warn` and the `source`-carrying records are reused rather than
      repeated. A value that will not decode gets the `warn` `decode` already gives a picture that will not. Write
      the doc comments to say what makes these safe beside the picture pair — D3's two independent mechanisms —
      rather than only what they do. Verify with tests over a **memory-backed** `Memo` that a value round-trips, that
      a value stored under `ENTRY_TTL` is the same lifetime a picture gets, and that `put_value` of a value that
      cannot serialise warns and stores nothing instead of panicking.

## 3. The key, and the pair it belongs to

- [x] 3.1 Add `identity_of_data(identity, operation)` beside `identity_after` in `inference/chain.rs` per design.md
      D4, folding the identity, the operation's cache tag and a fixed word discriminator that is **not** a
      `ChannelDepth::tag()`. Write the doc comment to say why it sits beside the chain's derivation rather than in
      the driver that calls it: the property is a relationship between the two, and a property of a pair is not
      checkable from inside one of them. Verify with tests that the same image and operation give a stable key across
      calls, that a different image and a different operation each give a different one, and — the one that matters —
      that `identity_of_data` and `identity_after` **never** produce the same string for the same identity and
      operation at **either** depth, which is the test an edit removing the discriminator has to fail.

## 4. The driver reads and writes

- [x] 4.1 Add `cache: bool` to `ExecuteOptions` defaulting to `true`, carrying `ProcessOptions::cache`'s own
      reasoning: decided once for the whole run so a run cannot be served from the store and then decline to keep
      what it produced. Extend the manual `Debug` impl and have `Opai::execute` pass `self.inner.cache.handle()
      .as_ref()` down, exactly as `Opai::process` does. Combine the handle and the flag into one `RunCache` in the
      driver, **once**. Verify it builds and that a run constructed with `..Default::default()` still compiles
      unchanged, which is the source-compatibility property `ProcessOptions` documents.
- [x] 4.2 Add the hit arm to `inference/execute.rs` per design.md D6, **above** `pipeline::acquire` — cancellation
      checked first, then `get_value` on a blocking thread, and on a hit `progress.operation(0, ..).cached()` and a
      return carrying `ProviderReport::new(provider, std::iter::empty())`. Write the module header's "what it
      deliberately does not do" paragraph to say the lookup's *position* is the load-bearing part and why. Verify
      with the fake backend that a hit acquires **nothing** — asserted against the fake's acquisition log, not
      against the result — that it reports `ProviderVerdict::NothingExecuted`, and that a run cancelled before the
      lookup is not handed a stored result.
- [x] 4.3 Add the `put_value` after a successful run, on the blocking thread that already exists, under the same key
      4.2 reads. Verify that a miss followed by a second run is served — the model runs once across the two — and
      that a **failed** and a **cancelled** run each store nothing, so a later run is not served a result that was
      never produced.

## 5. Verification

- [x] 5.1 Test the driver against the fake backend for the properties `data-inference`'s replaced requirements name,
      none of which need a runtime: the same operation twice over one image runs the model once; a different
      operation over the same image and the same operation over a different image each run their own model; a served
      run's progress still reaches 1.0; `cache: false` runs the model every time and leaves the store byte-for-byte
      as it was; a store that cannot be opened, a read that fails and an entry that will not decode each cost a
      re-run and **no** error reaching the caller.
- [x] 5.2 Test `run-cache`'s added requirement where the two kinds of entry meet: a picture entry read with
      `get_value` and a value entry read through the picture path are **each** reported unreadable rather than
      decoded into the wrong kind, and both fold into a miss rather than an error. Drive it over a memory-backed
      `Memo` with entries written by the real `put`/`put_value`, not by hand-built bytes, so the test breaks if
      either encoding changes.
- [x] 5.3 Pin the new surface in `crates/opai/tests/public_api.rs` the way `ProcessOptions::cache` is pinned there —
      the `ExecuteOptions { cache, .. }` field named in a construction and read back — and verify `cargo test -p opai
      --test public_api` passes with `Opai::process`, `ProcessOptions` and `Enhanced` unchanged in that file.
- [x] 5.4 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` and verify
      all three are clean, then `cargo doc -p opai --no-deps` and verify no new rustdoc warning against `HEAD`.
      Verify the chain's own behaviour is untouched: every test in `inference::chain` passes with **no edit to any
      test**, which is what makes the new derivation a sibling rather than a change to the enhancement path. No
      frontend scripts — this change touches no frontend file.

## 6. What verification found

- [x] 6.1 Add the `debug` record a **computed** analysis writes, beside the served one 4.2 added, both naming the
      operation and the slot — `logging`'s requirement was written over an enhancement's steps because those were
      every read of the store there was, and a second reader of the same store inherits the same silence. Broaden
      that requirement in `specs/logging/spec.md` rather than leaving the record to answer to nothing. Verify with
      the fake backend that a miss records `computed` and not `served`, that the hit its write produces records the
      reverse, that both name the **same** slot, and that neither appears at the default verbosity.
- [x] 6.2 Extend the crate-level cache section in `lib.rs` — the first thing a consumer of `opai` reads, and it
      described a store of photographs keyed on pixels, which is now half of what it holds. State the second kind of
      entry, what it is keyed on, that the saving is the session rather than the arithmetic, that the two kinds
      cannot be read as one another, and name `ExecuteOptions::cache` beside `ProcessOptions::cache`.
- [x] 6.3 Carry the `image-cache` → `run-cache` rename into the code: `cache::ImageCache` becomes `cache::Store`
      (`RunCache`, a run's view of it, is unchanged), and the prose sweep covers every comment, doc and log message
      in `opai` and `perf` that called it an image cache. Verify no test or spec pins one of those message strings,
      and record the rename in proposal.md — a capability whose name contradicts its newest requirement is how the
      next reader concludes there are two stores.
- [x] 6.4 Drive `data-inference`'s "a different operation over the same image" and "the same operation over a
      different image" **through the driver** rather than over the derivation: the test helper takes the source and
      the operation, and the two `NewYork` precisions are two operations. A driver that derived the right key and
      then read under another passes the key-level test and fails this one.
