## Slices

This change is **not** part of a series. `opai::initialize()` was delivered as the four-slice
`add-init-*` series, all of which are archived:

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 0 | `add-init-0-rust-sak` | `rust-sak` resume/extract-progress work and the `26.9.0` pin. | nothing | **done** (archived `2026-09-11`) |
| 1 | `add-init-1-runtime` | Config dir, pinned artifact table, progress types, manifest bookkeeping, the install pipeline, `Opai::initialize`. | slice 0 | **done** (archived `2026-09-11`) |
| 2 | `add-init-2-gpu` | NVIDIA detection, CUDA/cuDNN/TensorRT installs, loader search path including the Linux re-exec, the provider report. | slice 1 | **done** (archived `2026-09-11`) |
| 3 | `add-init-3-ort-env` | `ort` at `load-dynamic`/`api-26`, the library path on the descriptor, the eagerly-started environment, process-lifetime semantics. | slice 2 | **done** (archived `2026-09-11`) |

This change refactors what those four built. It is self-contained, fits inside the 15-task ceiling,
and nothing is queued behind it.

## Context

A `/simplify` review covering the whole repository produced findings in four categories. The
mechanical ones were applied directly in the cleanup pass that preceded this change:

- the loader's directory list derived from the release table rather than written out a second time;
- the every-launch `stat` of every installed file moved onto a blocking thread;
- `spawn_blocking` moved out of `deps/install.rs` into its own `task` module;
- `Opai::install` returning the directories it resolved, so `initialize` stops re-deriving one;
- roughly eighty lines of duplicated test scaffolding collapsed onto shared helpers;
- the self-re-invoking runtime tests no longer parsing libtest's summary line;
- `clippy.toml` enforcing that `runtime::start` is the sole caller of `ort::init_from`, which design
  D9 previously required in prose only.

What remains needs types to change, which is this change.

## Decisions

### D1 — The role goes on the row, as data, not as a `match`

`Release` already carries `dir` and `progress` on the row rather than at the call site, and
`artifact.rs` argues why at length: the alternative is a `match` elsewhere that has to be kept in
step with the table. That argument applies unchanged to the two things still missing from it.

```rust
pub(crate) struct Release {
    // ...
    /// The loadable library, or `None` where the dependency is a directory rather than a file.
    pub(crate) lib: Option<&'static str>,
    /// The execution provider installing this unlocks. `None` for the runtime, which is not one.
    pub(crate) provides: Option<Provider>,
}
```

`lib` becomes per-platform-nullable rather than an empty string. Note `Artifact::lib` is the
per-platform field and `Release` the per-dependency one — the `Option` belongs on `Artifact`, since
"has a library" is a property of the published archive.

**The cost, recorded:** the runtime always has a library, and the type no longer says so.
`initialize` needs the name to point the loader at, so it ends in an `expect` on a row that is
`Some` for every platform the runtime is published for. That invariant is held by `release.rs`'s
`the_runtime_descriptor_carries_its_platform_library_name`, which asserts it on all five — not by
the type, which makes it the same kind of hand-held invariant this change removes elsewhere. Kept
anyway: the alternative is a non-optional `lib` on a second row shape, which is the `enum Role`
rejected below. So the test is load-bearing rather than incidental — it is what stops a newly
pinned platform with no library from reaching that `expect`.

**Rejected:** a full `enum Role { Runtime { .. }, Provider { .. } }`. It expresses the same thing,
but it makes the table rows structurally different from one another for no gain — every consumer
wants one of the two optional fields, not a match over shapes. Two `Option`s keep the table a table.

### D2 — `select_at` returns a `Plan`, and the provider report is derived from it

```rust
pub(crate) struct Plan {
    pub(crate) runtime: Descriptor,
    pub(crate) gpu: Vec<Descriptor>,
}

impl Plan {
    fn providers(&self) -> SupportedProviders { /* base ∪ each gpu row's `provides` */ }
    fn all(&self) -> impl Iterator<Item = &Descriptor> { /* runtime first, then gpu */ }
}
```

The install loop iterates `all()`, so install order is unchanged: runtime, CUDA, cuDNN, TensorRT.
`initialize` takes `plan.runtime` by name. `SupportedProviders` is computed rather than assigned, so
"a provider is claimed only where its library was actually installed" stops being a property the
tests check and becomes one the type guarantees.

The existing test asserting `report.cuda == plan.contains("cuda")` stays, but changes meaning: it
now checks the derivation is wired up, not that two hand-maintained things still agree.

**Ordering stays load-bearing and stays documented:** cuDNN after CUDA because its libraries link
against the CUDA runtime, TensorRT last because it is larger than the other three together. That
lives in the table's order, which `Plan::all` preserves.

### D3 — The Linux re-exec is out of scope, and here is why it is written down anyway

`libpath::apply`'s Linux arm replaces the process image so glibc picks up `LD_LIBRARY_PATH`. The OS
difference driving it is real and irreducible: glibc snapshots that variable at exec time, where
Windows' `LoadLibrary` consults `PATH` at call time.

But re-exec is not the only answer to it, and **design D6 evaluates only the Windows alternative**
(`SetDllDirectory`/`AddDllDirectory`) — the Linux one was never considered. The alternative is to
`dlopen` the NVIDIA libraries with `RTLD_GLOBAL | RTLD_NOW` in dependency order immediately before
`ort::init_from`: the loader satisfies `DT_NEEDED` entries from already-loaded objects by SONAME
before it consults any search path, which is exactly what upstream ONNX Runtime's own
`preload_dlls()` does for this problem. It would fit this codebase's own grain as a
`preload: &[&str]` column of SONAMEs on the row, resolved from the table rather than by a `match`.

What it would delete: the `unsafe` on a public entry point, the "first statement of `main`, before
any other thread, before the logging setup" contract that no signature can enforce and both front
ends must remember, the `OPAI_REEXEC` guard variable, the documented file-descriptor hazard, and
`tests/reexec_linux.rs`.

**It is not proposed here.** It is unverified against TensorRT's plugin loading, which `dlopen`s its
own plugins and may not be covered by preloading the core libraries; and unlike everything else in
this change it would alter how the application starts, which is not a refactor. The right next step
is to answer it in D6 with the hardware in hand. Recorded so it is not lost.

### D4 — `InitError::Io` carries a path; runtime shutdown gets its own variant

```rust
Io { path: PathBuf, source: std::io::Error },
Cancelled,  // the Tokio runtime shut down before blocking work could run
```

Every `Io` construction site is adjacent to the path it was working on, so filling it in is local.
The cost is dropping `#[from]`, which means roughly ten `?` sites need `.map_err(..)`. That is the
point: a bare `?` is precisely how the path got lost.

`Cancelled` separates "the runtime is shutting down" from "the disk failed" — the one distinction a
caller here can act on, and today the two are indistinguishable.

**Not changed:** `InitError::Fs` stays `#[error(transparent)]` over `rust_sak::fs::FsError`, which
carries its own context. And nothing in the enum is too *fine*: `RuntimeLoad` / `RuntimeStart` /
`RuntimeUnavailable` map to three genuinely different front-end responses — reinstall, report,
restart the process — and the tests assert on that distinction.

### D5 — `Platform` as a typed key

`asset_platform(os, arch) -> Option<String>` produces `darwin_arm64`; `Artifact::platform` must
match one. Nothing connects them, so a typo in the table is not a compile error — it silently makes
that platform unpublished, and the first person to find out is a user on it.

The cleanup pass added a test asserting every pinned `platform` is producible by the mapper. This
change replaces the test with the property: a `Platform { os: Os, arch: Arch }` pair with a
`const fn` spelling the segment, so `asset_platform` becomes `Platform::from_consts` and an
unspellable platform cannot be written down.

Keep the existing test anyway — it costs nothing and would catch a mapper that stops being total.

## Platforms and providers

The refactor is platform-agnostic and changes no platform-specific behaviour. Validated the same way
the current code is: the pinned table's platforms are all exercised from whichever runner the suite
runs on, because `from_release_at` takes `os`/`arch` as arguments. Untested as before: real NVIDIA
hardware — no CI runner has any — so the CUDA/TensorRT selection branches are covered against
constructed `GpuInfo` values, not against a card.

## Ownership and lifetimes

Nothing here touches model sessions, memory budgets or the ONNX environment's lifetime. `Plan` and
`Descriptor` are plain owned data built during selection and dropped after the install; the
environment's process-global lifetime and the `FAILED_LOAD` latch are unchanged.
