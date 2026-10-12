## 1. What every later task asserts against

- [x] 1.1 Promote `records_of` out of `lib.rs`'s test module into `crates/opai/src/logging/mod.rs`
      under `#[cfg(test)]`, taking the filter directives as an argument so a test can drive the same
      code at `info` and at `debug` (design D10). Carry its two constraints across as documentation
      rather than as folklore — it asserts on the file's text because a collector *"could pass while
      the file said something else"*, and `set_default` binds to the calling thread, so a record
      emitted from `spawn_blocking` is not seen. Verify `lib.rs`'s existing record tests pass
      unchanged against the promoted helper.

## 2. The enhancement run

- [x] 2.1 Bracket the run in `inference/chain.rs::process`: an `info` when it begins carrying the
      operation count, the operation ids, the source's identity hash and the resolved depth, and an
      `info` when it ends carrying the elapsed time (design D3). Rewrite the module header's *"What
      is deliberately not here — Logging"* entry to describe what is now there. Verify a test over
      the fake `Backend` asserts both lines and that `duration` renders as `1.523s` rather than as a
      field set.
- [x] 2.2 Instrument `chain::run`'s per-operation loop — `debug` for a step served from the store and
      for a step computed, `warn` when a stored prefix will not decode and the run rewinds, `warn`
      naming the operation and the error when a step fails — plus the `warn` in `chain::plan` when a
      chain is refused, which is Go `inference.go`'s "operation type not supported" (design D2, D4).
      Verify tests over the fake backend assert a hit, a miss, a failed step and a refused chain, that
      the run's own failure is distinguishable from a step's, and that the same run at `info`
      produces none of the per-step lines.

## 3. The image cache

- [x] 3.1 Record the outcomes `cache.rs` currently discards (design D4): `warn` when a result will
      not encode or the write **breaks**, `debug` when the store **declines** it — the declined/broken
      split is what D4 is for, and the level is where it lands; `warn` when a read returns a store
      error, distinguished from a plain miss; `warn` when a stored entry will not decode; `debug`
      once per run where the run has no store at all. Keep `put`'s signature and `get_bytes`'s
      `.ok().flatten()` return exactly as they are — the distinction lands in the log, not in an API.
      Verify tests over a store that fails each way assert one record each.

## 4. Sessions, providers and the caches they compile into

- [x] 4.1 Instrument the registry (design D6): `debug` for a resident hit, `info` when a build starts
      and when the session is ready with its artifact, provider and duration, and `info` for
      evictions from the sweeper and from `clear`, naming how many were released. Verify tests
      through the existing fake builder assert each, and that two requests for one artifact produce
      one build record and one hit record.
- [x] 4.2 Record the downgrade in `Sessions::session` at `warn` — artifact, the provider that failed,
      the error — with the CPU build that follows carrying `requested` beside `provider` so the pair
      reads as one event (design D6). Rewrite `SessionHandle::requested`'s comment, which says it is
      *"the one place a downgrade is reported at all: this project has no logging"*, to say the log
      reports it too and why the accessor still matters. Verify the existing downgrade test also
      asserts the two records.
- [x] 4.3 Enter a span carrying `artifact` and `provider` around the synchronous build in
      `sessions/build.rs::build`, so ONNX Runtime's own diagnostics say which model they are about
      (design D5). Document what it covers and what it does not — records emitted on the build thread
      are decorated, records from the runtime's own worker threads are not. Verify a test emitting an
      `ort`-target event inside the span asserts both fields on the rendered line.
- [x] 4.4 Record the two discards that cost a user minutes of recompilation: the TensorRT timing cache
      dropped after a failed build in `sessions/build.rs::serialized`, and the engine tree emptied by
      `caches::invalidate_for_runtime` when the runtime tag moves, at `info` naming the tag that
      replaced it (design D1). Verify the existing timing-cache and invalidation tests assert the
      records alongside the filesystem effects they already check.
- [x] 4.5 Add the teardown and hardware records that have a Rust counterpart: `Opai::release_sessions`
      saying what it released, `gpu.rs::adapters` at `warn` when the probe fails and is memoised as an
      empty list, and `lib.rs::select_at` at `info` when a GPU is too old for the pinned release.
      Verify a test asserts the release record, and that a probe returning an error still initializes
      on the CPU with the warning present.

## 5. The installers and the listing

- [x] 5.1 Instrument `deps/install.rs` (design D8): `debug` for a dependency already present and for
      an archive that could not be sized, `info` for installing, extracting and ready, and `warn`
      naming the artifact when a transfer is repeated after a mismatch and when an install fails.
      The repeat record must say *why* the clean pass is happening, which `acquire` knows and the
      reference's four separate warnings do not. Verify tests against `TestServer` assert the records
      on a first install, a second install that transfers nothing, and a mismatch that recovers.
- [x] 5.2 Instrument the model install in `deps/model/mod.rs` with the same three, keyed on the
      artifact id so one `grep` finds the install, the build and every run that used it — and record
      the refusal of a model absent from the manifest as an error on the path that fails, not the
      reference's warn-and-download (design D1, `model-install`'s existing divergence). Add the
      `warn` in `deps/manifest.rs::read` for a malformed or wrong-schema record. Verify tests assert
      each, including that the refusal record names the artifact.
- [x] 5.3 Record the listing's three-way resolution in `deps/model/manifest.rs::resolve_once` (design
      D9): `debug` with the entry count when the live listing is read, `warn` naming the failure when
      the cached copy is used, `warn` when the compiled-in listing is used, and `warn` when the
      manifest cannot be written back to the cache. Verify the existing fallback tests — unreachable
      listing, no network at all, undecodable cache — each assert their record.

## 6. Volume, comments and verification

- [x] 6.1 Add the upscale pass record at `debug` in `inference/upscale.rs::run_passes` — the pass
      index, the total and the dimensions produced — add **no** record inside `run_tiled` or the grid,
      and pin the bound that decision exists for: the same enhancement at the default level over a
      small image and over one many times larger produces the same number of records, and a chain of
      several operations is bounded by the operation count rather than by the work each one did
      (design D2). Verify the test fails if a record is added inside the tile loop.
- [x] 6.2 Sweep every doc comment this slice makes untrue, in one pass: `cache.rs`'s module note
      (*"the first thing to reinstate on the day logging lands"*) and its repeat on `put`, `lib.rs`'s
      two module notes (*"None of it is logged, because this crate has no logging yet"* and the cache
      paragraph), and `sessions/mod.rs`'s "Nothing here is logged" section with its two accessors'
      comments. Verify no occurrence of "no logging" or "when logging lands" remains in
      `crates/opai/src`, and that each rewritten section describes records the tasks above actually
      added.
- [x] 6.3 Run `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` across the workspace,
      then process a real image through `cli` against a real runtime and confirm `opai.log` reads as
      an account of the run: the pair of records, the provider it landed on, and nothing per-tile.

      **Done, and the live leg lives in `inference/live.rs` rather than in `cli`** — which has no
      processing surface to drive, so `opai`'s own live suite is the only place a real photograph
      meets a real runtime. Run by hand on macOS arm64 as
      `cargo test -p opai --lib a_real_run_leaves_an_account_of_itself_in_the_log -- --ignored`:
      a 640×640 Kyoto 4x produced **six** records at the default level — the run's pair
      (`duration=11.590607s`), the session build's pair (`provider=CoreML requested=Auto`), and the
      model install's pair — and not one line per tile or per pass. The test stays `#[ignore]`d,
      since it downloads the pinned runtime and Kyoto's weights.
