## Context

See `proposal.md` for the motivation. After slice 1, `opai` loads the runtime through `rust-sak` but still builds
sessions itself:

```
sessions/mod.rs        Sessions::request: walks the ladder, skips declined rungs (Auto), OOM retry, downgrade records
sessions/resident.rs   SessionCache: resident Arc<Mutex<Session>>, single flight, 15-min sweep, decline set
sessions/build.rs      build(): span, model-file pre-check, TENSORRT_BUILD mutex, timing-cache discard,
                       attach providers via with_arbitrary_config + error_on_failure, BuilderSettings, OOM marking
sessions/caches.rs     engines/<artifact>/, engines/.timing/, engines/.version stamp
providers/mod.rs       ExecutionProvider, Accelerator, SupportedProviders (public)
providers/options.rs   resolve_chain, ladder, AUTO_ORDER, per-provider option maps, SessionPlan
providers/profile.rs   EpProfile and its enums; ~22 model files declare one
runtime/diagnostics.rs ORT logger: severity correction, repeat throttle, per-thread OOM counter
```

`rust-sak` `ai` (tag `26.11.0`) already has `ExecutionProvider`, `Supported`, `AUTO_ORDER`, `ladder`, `Tuning`,
`provider_options`, `trt_workspace_bytes`, `CachePaths` and `session`. Its option maps and session settings are
OPAI's, key for key, and its `ladder(&[p], supported)` gives the same rungs as OPAI's `ladder(p, supported)` for every
single request. Its `session()` walks the whole ladder itself, which OPAI cannot use, because the decline latch, the
OOM retry and the resident cache sit between rungs.

The gaps between the two implementations:

| OPAI does | `rust-sak` today |
|---|---|
| Builds one rung at a time; the caller decides the next | Walks every rung inside `session()` |
| Marks a failed build as out-of-memory (logged record on the build thread, or the error text) | No classification |
| One timing cache shared by every model | `CachePaths::under` puts it under each model's folder |
| Serialises TensorRT builds; empties the timing cache when one fails | Neither |
| Probes CUDA device memory once per process | Probes on every `session()` that may attach TensorRT |

Two pieces of existing spec text disagree with the code, and this slice does **not** change either:
`model-sessions` says "A downgrade SHALL NOT be latched", while the code latches declines under `Auto`. And
`execution-providers` says TensorRT gets "a 4 GiB workspace ceiling", while the code gives a quarter of the card,
capped at 4 GiB. Behaviour stays exactly as the code has it. The spec text is for a later change to reconcile.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `adopt-rust-sak-ai-1-deps` | Runtime and NVIDIA installs, GPU selection, library path and runtime load on `rust-sak` `ai`; `ort` rc.15; shared `io.vinicius.ai` root; old folders removed | - | done |
| 2 | `adopt-rust-sak-ai-2-sessions` | Session building, provider options, ladder and tuning on `rust-sak` (single-rung build with OOM classification, shared serialised TensorRT timing cache, explicit cache paths); `ExecutionProvider`/`SupportedProviders` re-exported; `EpProfile` → `Tuning`; per-provider engine folders. OPAI keeps rung policy, resident cache, OOM retry, decline latch | 1 | **current** |
| 3 | `adopt-rust-sak-ai-3-models` | Generic Hugging Face model installer in `rust-sak` `ai` (named files from a folder, API-hash verified, resumable, progress, trust-local option); old model files deleted and re-downloaded (TODO-marked); compiled-in and cached listings dropped; `ModelTrust::LocalFiles` mapped onto trust-local | 1 | pending |

Each slice ships its `rust-sak` additions under a new tag first, then the OPAI migration that uses them.

## Goals / Non-Goals

**Goals:**

- OPAI holds no code for describing providers, resolving rungs, building provider option maps, attaching providers,
  applying session settings, serialising TensorRT builds or recognising an out-of-memory build. What stays in OPAI is
  policy: which rungs to try, what stays resident, when to retry, what to record.
- Every model runs with exactly the options it runs with today. A test compares old and new option maps for every
  catalogue entry before the old code is deleted.

**Non-Goals:**

- Changing the ladder, the decline latch, the OOM retry, the resident cache, the 15-minute sweep or the single
  flight. Their behaviour and their records stay.
- Model installs (slice 3). `deps/model/` and the per-model engine invalidation at install are unchanged, except that
  the folder they empty now has provider subfolders in it.
- Reconciling the two stale spec texts named above.

## Decisions

### D1. `rust-sak` gains a single-rung build; `session()` is built on it

New public API in `rust-sak` `ai`, settled during implementation but with this behaviour:

- `SessionRequest` takes `cache: CachePaths` in place of `cache_dir`. `CachePaths::under(dir)` stays as the way to
  get the per-model default. **Breaking for `rust-sak`**, whose only session consumer is OPAI. The README is updated.
- `build(&request, rung: &[ExecutionProvider]) -> Result<Session, RungError>` builds on exactly that rung, and an
  empty rung means the CPU. `Auto` and `Cpu` inside a rung are ignored, just as `ladder` never produces them.
  `RungError` carries the rung, the runtime's reason and `out_of_memory: bool`. The build runs **on the calling
  thread**, which is a documented contract, since OPAI's `session_build` span depends on it (D6).
- `session()` becomes `ladder` plus a loop over `build`, with the same warnings and the same `SessionError`.

Alternative considered: give `session()` a callback for "skip this rung" and "retry this rung". Rejected, because
it moves OPAI's policy (the decline set, releasing unused resident sessions) into a callback shape that only OPAI
needs. A single-rung primitive is the smaller and more general API.

### D2. Out-of-memory classification lives in `rust-sak`, fed by its own logger wrapper

`load_with` always installs a `rust-sak` logger. For each record, it increments a thread-local counter when the
message reports a failed device allocation (`OutOfMemory`, `out of memory`), then forwards the record to
`LoadOptions::logger` or, when that is `None`, to `tracing` in `ort`'s own shape. `build` reads the counter before
and after the attempt. A failed rung is `out_of_memory` when the counter moved, or when the error text contains
`out of memory`, `outofmemory` or `failed to allocate` (ignoring case), and **never** on the CPU rung. These are
exactly OPAI's two current tests, moved.

Why `rust-sak`: knowing that TensorRT's "failed to create engine" meant the card was full is a fact about the runtime
and its providers, not about OPAI. Any app with a retry policy wants it.

The counter is thread-local, `const`-initialised and has no destructor, as OPAI's is today, so reading it from the
runtime's callback during thread teardown cannot panic across the FFI boundary. OPAI's `diagnostics` keeps the
severity correction (a skipped TensorRT tactic logged at `WARN`), the repeat throttle and `flush`, and loses its
counter. A skipped tactic still counts as OOM because its message contains `OutOfMemory`, which keeps today's
behaviour.

### D3. TensorRT builds serialised, timing cache emptied on failure, in `rust-sak`

A process-wide `static Mutex<()>` in `rust-sak` is held for the whole of any `build` whose rung contains TensorRT.
A poisoned lock is taken anyway, since it guards an ordering and not data. If that build fails, `cache.timing` is
emptied (the folder stays, its contents go) and an `info` record names the path. A rung without TensorRT neither
waits nor discards. These are OPAI's `serialized()` semantics and tests, moved.

The lock is in-process only. That is enough for OPAI, whose timing cache is under its own configuration folder,
which the single-instance claim gives to one process. Another app pointing two processes at one timing folder would
need more, and that is out of scope.

### D4. Engine layout: per-provider subfolders, shared timing

`caches::resolve` returns `rust_sak::ai::CachePaths`:

```text
<config>/engines/<artifact>/tensorrt/   TensorRT engine cache        (CachePaths::under(engines/<artifact>).tensorrt)
<config>/engines/<artifact>/coreml/     CoreML compiled model        (CachePaths::under(engines/<artifact>).coreml)
<config>/engines/.timing/               shared TensorRT timing cache (overrides .timing)
<config>/engines/.version               "<runtime version>;layout=2"
```

Per-provider folders are the better layout on their merits: each provider owns its directory, so one can be cleared
or inspected without touching the other, and two providers' file names can never collide. The shared timing cache is
the better choice because timings carry between graphs (see the `model-sessions` delta). The `rust-sak` default of
a per-model timing cache gives that up, so OPAI overrides the one field.

The per-model invalidation at install empties `engines/<artifact>/`, which now holds the provider subfolders, so
there is no change there. `rust-sak` creates the folders only when a compiling rung is tried, so a model only ever
run on the CPU leaves none behind.

**Discarding the old flat layout.** The stamp gains a layout marker: `invalidate_for_runtime` compares against
`format!("{runtime_version};layout=2")`. The existing stamp is the bare `1.30.0`, which no longer matches, so the
first launch empties the tree once (timing cache included) and writes the new stamp. That is the spec's "a record
that names no layout reads as a different layout". A durable stamp and not a one-off cleanup, so a later layout
change costs one constant bump.

Alternative considered: keep the flat `engines/<artifact>/` with fully explicit paths, so nothing recompiles.
Rejected on the merits above. The one-time recompile is a cost, not a reason.

### D5. Public types are `rust-sak`'s

`lib.rs`: `pub use rust_sak::ai::{ExecutionProvider, Supported as SupportedProviders};`. `providers/mod.rs` keeps
only what is OPAI policy, if anything. `Accelerator`, `BUILT_ON`, `with_webgpu` and the `From<Supported>` conversion
go. Sessions read `rust_sak::ai::Supported::detect(cuda, tensorrt)` after the load, so WebGPU no longer has to be
patched in later (`set_webgpu` goes). `Supported::available()` lists in the same order as today.

`InitError::UnknownProvider { name }` stays, with `From<rust_sak::ai::UnknownProvider>`, so `gui/src/setup.rs`'s match
still compiles. `perf/src/cli.rs` maps the new error type. Differences that remain: `rust-sak`'s `Supported` is not
`#[non_exhaustive]`, and its doc comments differ. Neither changes the wire form, which a test pins (the existing
`the_report_serializes_as_the_fields_a_front_end_reads` and the spelling tests keep running against the re-export).

### D6. What `sessions/build.rs` becomes

```
build(request) on a blocking thread:
  span session_build{artifact, provider = first of rung or CPU}  -- entered on this thread
  File::open(model)?  -> SessionError::Install (never retried)
  rust_sak::ai::build(&SessionRequest{model, providers: &[requested], supported, tuning, cache}, &rung)
    Err(RungError{out_of_memory: true, ..}) on a non-CPU rung -> SessionError::Build{source: OutOfDeviceMemory}
    Err(other)                                               -> SessionError::Build{provider, source}
  diagnostics::flush()
```

`Sessions::request` takes its rungs from `rust_sak::ai::ladder(&[requested], supported)`, filters declined ones under
`Auto` exactly as now, and keeps `build_releasing` (one retry after `release_unused` when `is_out_of_memory()`). The
downgrade `warn`, the OOM-retry `info`, the `PROVIDER_FALLBACKS` metric and the build start/finish records are
untouched. `BuildRequest` carries the rung, the `Tuning` and the `CachePaths` in place of a `SessionPlan`.

### D7. `EpProfile` becomes `Tuning`

Each model's `fn profile(..) -> EpProfile` returns `rust_sak::ai::Tuning`, with the same field names and the enums
imported from `rust_sak::ai`. The `&'static [&'static str]` node lists stay as `const`s in each model and are turned
into a `Vec<String>` where the tuning is built (`with_webgpu_cpu_nodes_at_fp16` and `cpu_and_gpu_at_fp16` stay as
free helpers over `Tuning` in `providers/`). `PROVIDER_OPTION_LIMIT` and its test stay. `TRT_BUILDER_OPTIMIZATION_LEVEL`
stays as the key Osaka's `trt_options` spells.

**Equivalence gate before deletion:** a test iterates the whole catalogue and checks that every
`(operation, provider)` produces the same option map and the same session settings through
`rust_sak::ai::provider_options` as through OPAI's current `options::plan`, using fixed paths. It is written first
against both implementations, and kept afterwards as a snapshot of every model's maps.

### Ownership and lifetime

- **Environment:** owned by `rust-sak`'s process-wide load state since slice 1, borrowed by every `build`, never torn
  down.
- **Session:** `rust_sak::ai::build` returns an owned `ort::Session`. OPAI's `SessionCache` wraps it in
  `Arc<Mutex<_>>` exactly as today, and it is freed when the last `Arc` goes. `rust-sak` keeps no reference to a
  session it built.
- **TensorRT lock:** a `rust-sak` static, held by the blocking thread for one rung's build, released on return or
  panic. It is never held across an `.await`, since builds run under `spawn_blocking`.
- **OOM counter:** thread-local in `rust-sak`, written only by the runtime's logger callback, read before and after a
  build on the same thread.
- **Device-memory probe:** a `OnceLock` in `rust-sak`, first filled by a build whose rung contains TensorRT. A machine
  that never attaches TensorRT never loads the CUDA driver for it.
- **Cache folders:** paths owned by OPAI (`caches::resolve`), created by `rust-sak` on a compiling rung, emptied by
  OPAI (per model at install, wholesale on a stamp mismatch) and by `rust-sak` (timing folder on a failed TensorRT
  build).

### Crates and features

`rust-sak` `ai` only, already enabled in `opai` since slice 1, at a new tag in `opai`, `gui`, `cli` and `perf`
together so the lock file holds one `rust-sak`. No crate is added. `gui` and `perf` change only where they name the
`from_str` error. `cli` changes only the tag. No logic moves into a front end.

## Risks / Trade-offs

- [CUDA/TensorRT rungs, TensorRT serialisation and OOM classification cannot run on macOS] → Their logic is moved
  with its pure tests (rung walking with a fake builder, serialisation with a fake build, OOM classification over
  constructed records and messages). Release gate: on Linux+NVIDIA and Windows+NVIDIA, a TensorRT build with the
  timing cache shared across two models, a forced failure that empties it, and an OOM retry seen in the log.
- [Every user recompiles every engine once] → Accepted. TensorRT costs minutes per model on first use. The existing
  `info` record names the reason ("…has been replaced") and is reworded to cover a layout change.
- [A drifted option for some model, silently slowing it] → The catalogue-wide equivalence test (D7) runs before the
  old code is deleted.
- [`rust-sak`'s logger wrapper changes the record shape `logging::format` depends on] → The wrapper forwards to the
  caller's logger untouched, and OPAI always passes one. Its `None` branch is tested in `rust-sak` to produce
  `ort`'s shape.
- [Breaking `rust-sak` API (`SessionRequest::cache`)] → First-party, consumers pin by tag, and OPAI is the only
  session consumer. The README records it.
- **Platforms validated on:** macOS arm64 with CPU, CoreML and WebGPU, by the developer and by `rust-sak`'s tests.
  **Untested until the release gate:** Linux x64/arm64 and Windows x64/arm64 with CUDA, TensorRT and WebGPU.

## Migration Plan

1. `rust-sak`: D1–D3 and the cached probe. README updated, tests green, tagged after the user's go-ahead.
   **As released:** no new tag was cut. The existing `26.11.0` tag was moved, with the user's agreement, from
   `0b23117` (slice 1) to `516b014`, which adds D1–D3. OPAI is `rust-sak`'s only session consumer, and every lock
   file pins the exact commit, so slice 1's lock (`26.11.0#0b23117`) still resolves: `0b23117` is an ancestor of
   `516b014`. The `rust-sak` README's "Breaking in 26.11.0" note refers to the moved tag.
2. OPAI: move the lock to the new `26.11.0` commit (the tag in every `Cargo.toml` is unchanged), add the
   catalogue-wide equivalence test against both implementations (D7).
3. OPAI: re-export the provider types (D5), switch to `Tuning` (D7), replace the build and plan code (D6), move the
   layout and stamp (D4), drop the OOM counter (D2).
4. Verification on macOS (all models, FP32 and FP16, CPU and CoreML, through Rust), then the NVIDIA hardware gate.

Rollback: revert the OPAI commit. The earlier release finds a `;layout=2` stamp it does not recognise, empties the
tree once and recompiles into its flat layout. Nothing else on disk differs.
