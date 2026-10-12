## Slices

Seven changes, in order. Single-instance came first because the changes after it are designed
against its guarantee, and logging came next because the benchmark ahead of it wants a sink to see
the library through. **This change is perf slice 2, change 6 of the seven.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** (archived `2026-09-14`) |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | **done** (archived `2026-09-14`) |
| 3 | `add-logging-1-file` | The sink: `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, daily rotation, the `---` session divider, the session header, the per-record `app=` field, `APP_NAME`, and the initialization path instrumented | 1 | **done** (archived `2026-09-15`) |
| 4 | `add-logging-2-records` | The instrumentation sweep: the enhancement run, the session registry, the installers, the image cache and the model listing, against the records the Go library emits | 3 | **done** (archived `2026-09-15`) |
| 5 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | **done** (archived `2026-09-15`) |
| 6 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 5 | **current** |
| 7 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 6 | pending |

Slice 7 is what makes a sweep watchable; everything below is deliberately shaped so that adding it
is an alternative renderer over the same sweep rather than a change to the sweep — see D8.

## Context

See proposal.md — Why. What shapes this design is six things read out of the code rather than
assumed:

- **Only Kyoto runs.** `inference/chain.rs`'s `plan_one` refuses the seven other families ("this
  family's pipeline is not written yet"), Tokyo and Saitama ("its execution-provider profile has not
  been measured yet"), Osaka ("the diffusion upscaler needs a pipeline of its own"), and a scale no
  pass sequence covers. So a harness shaped like the reference's would spend its default invocation
  reporting twenty refusals. The refusals are also **free** — `plan` runs over the whole chain
  before anything is installed — which is what makes attempting one and dropping it cost nothing
  (D2).
- **The library publishes its own catalogue.** `catalogue()` returns every family with its variants,
  their published precisions and its parameter's name and bounds, and
  `Operation::from_catalogue{,_with}` turns a row back into a runnable operation. The reference needs
  a 21-row table and four generic adapters because Go will not convert `func(float64,
  types.Precision) tokyo.OpUpTokyo` into a `types.Operation`; here `Operation` is one enum and the
  catalogue is already data. The harness's model table is therefore **nothing at all**.
- **Codenames are unique across families.** No codename is published by two families, so `perftest
  kyoto` resolves without a family qualifier by scanning the catalogue — checked, not assumed.
- **Nothing has ever called `initialize` or `process`.** `crates/cli/src/main.rs` installs a log sink
  and prints a version; `crates/gui/src/lib.rs` installs a log sink and builds a Tauri app. This
  crate is the first consumer of either.
- **`release_sessions` frees synchronously when nothing is in use**, because the cache's own `Arc` is
  then the last reference — which is exactly the property the measurement loop needs and is why
  slice 5 made it public. See D5 for what that means for this loop's ownership.
- **The log is a file, not a stream.** `logging::init` is latched to one sink writing
  `<config>/logs/opai.log`. There is no second subscriber to install, so the reference's `-v`
  "print the library's debug log to stderr" has no counterpart here and becomes something else
  (D10).

## Goals / Non-Goals

**Goals:**

- One command that produces a number worth quoting, and a header that makes it interpretable a month
  later in an issue thread.
- A sweep whose every stage is honest about what it did and did not include — which is mostly a
  matter of what is *excluded* from a timing, and is why D5 and D6 are the load-bearing decisions
  here rather than the report.
- A harness that is a consumer of the public surface and nothing more. If something turns out to be
  missing, that is a finding to report rather than a reason to reach into the crate or to widen its
  surface on a benchmark's account (D2).
- Everything that is not the sweep — selection, statistics, formatting, exit status — testable on a
  runner with no runtime, no GPU, no model file and no network.

**Non-Goals:**

- **The live view.** No spinner, no per-tile progress bar, no ETA, no `insert_before`, no TTY
  detection and no `--plain` flag, because with one renderer there is nothing to opt out of. Slice 7.
- **Comparing two sweeps.** `--json` makes a sweep comparable by a program; writing the program that
  compares them is not this slice, and no baseline file, threshold or regression check is designed
  here.
- **Benchmarking anything but a single-operation chain.** One model per measurement, as the reference
  does. Chains of two operations are a different question — mostly about the store between them —
  and inventing an answer for it now would mean a flag nobody has asked for.
- **Detection and face recovery.** No pipeline, and no public way to run an operation whose result is
  data rather than an image, so there is nothing to time. The reference's untimed detection pass and
  its `faceEntry` adapter have no counterpart here.
- **Memory figures.** Neither the per-pool budgets the reference's header reports, nor peak resident
  memory per model. The first does not exist in this project; the second is a measurement of a
  different kind and would need its own design.

## Decisions

### D1. A fourth workspace member, `crates/perf`, and not a subcommand of `cli`

The project context settles the shape — *"later consumers (a benchmark binary, for instance) are
added as further crates depending on `opai`, never by widening a front end"* — and the reasoning is
worth restating because the alternative is genuinely tempting: `cli` already exists, already
initializes a log sink, and a `perftest` subcommand would cost no new manifest.

It would also put `clap`, a table renderer, a JSON serializer and a statistics module into the binary
a user runs to enhance a photograph, and it would make the benchmark's argument surface part of the
application's. The two have opposite audiences: `cli` is shipped, `perftest` is a developer tool that
downloads gigabytes and writes into the shared cache. Keeping them separate is also what lets this
crate take dependencies — `comfy-table`, `serde_json` — that the shipped CLI does not carry.

The package is named `perf` and its binary `perftest`, matching the reference's `cmd/perf` producing
`perftest`, so that the command in a README written there is the command here.

### D2. The harness has no notion of which models are implemented, and `opai` gains nothing

Nothing is added to the library, and the harness keeps no list, no flag and no vocabulary for the
fact that `chain::plan` currently runs one model out of twenty-one. A sweep learns it the only way it
needs to: it asks the library to run the operation and reads what comes back.

This is a reversal of an earlier draft of this design, which added `Operation::supported()` to `opai`
— a public wrapper over the private `plan_one` — so the harness could select what to sweep without
attempting anything, and so `perftest list` could mark each row runnable or not with the reason. It
was rejected, and the reasoning is the thing to keep: **every model will run eventually**. The
refusals are scaffolding for pipelines that are being written, not a property of those models, so an
API describing them would be a permanent public surface for a temporary state — the kind of thing
that survives long after the condition it described is gone, because by then something depends on it.
The same argument disposes of a harness-side list, which would additionally be a second copy of a
decision the library already makes and wrong the day after the next pipeline lands.

What it costs is real and small: the harness cannot say *why* a model would be declined until it
attempts one, so `perftest list` prints the catalogue unannotated and finding out means naming a
model and reading the error. A sweep given no names attempts every published variant and drops the
ones the library declines, which costs one `process` call each — microseconds, because the refusal is
checked over the whole chain before anything is installed or opened.

And when every family has a pipeline, none of this has anything to remove: the sweep attempts
everything and nothing is dropped.

**What this forecloses.** A GUI building its variant chooser from `catalogue()` has the same question
— which of these can I offer? — and will still have it. That is a question for whichever slice
writes that chooser, against however much of the catalogue runs by then, and not something to answer
speculatively from a benchmark's requirements.

### D3. The selection is the catalogue, and the value comes from the parameter's published name

Resolution runs entirely over `catalogue()`:

- **No names given** → every published variant, at the requested precision, attempted in catalogue
  order; the ones the library declines are dropped from the report, with a line saying how many and
  that naming one shows why. Today that leaves Kyoto. A refusal is not a measurement and does not
  belong in a table nobody asked to see it in.
- **Names given** → each resolved by scanning the catalogue for the codename (unique across families,
  checked), built at the requested precision, and attempted whatever the library makes of it — so
  naming `stockholm` reports the refusal rather than silently dropping it.
- **A name no family publishes** → an error naming it and pointing at `perftest list`, before
  anything is initialized.
- **A codename published at a precision that was not asked for** — Osaka publishes `int8` and
  `fp16` and no `fp32` — → `from_catalogue*` returns `None`, and that is reported as *"the catalogue
  publishes no kyoto at int8"* rather than as a failure of the model.

**The parameter value is chosen by the name the catalogue publishes for it, not by the family.** The
five parameterized families publish three parameter names — `scale` (upscale), `strength` (denoise,
sharpen) and `bias` (light adjustment, colour balance) — and `ParameterEntry` carries that name
beside the bounds `Scale::new`, `Strength::new` and `Bias::new` actually enforce. So the harness has
three flags named after them, `--scale`, `--strength` and `--bias`, and picks between them by reading
`ParameterEntry::name`. A family added later with a parameter the harness has no flag for is then a
loud, specific failure — *"the catalogue publishes a parameter this harness has no flag for:
`radius`"* — rather than a silently wrong number.

This is a naming divergence from the reference, whose `--intensity` covers both the strength and the
bias families. It is worth it because those two are not the same parameter: `Strength` runs `0..=2`
and `Bias` runs `-1..=1`, and one flag over two ranges means a value that is valid for half the
selection. The flags' validators come from the catalogue's published bounds, so neither can offer a
value the constructor would refuse.

### D4. The chain is one operation, and the input's identity is what keeps the runs honest

Each measurement runs `opai.process(&picture, &[operation], Some(options))` — one operation, as the
reference does.

`ProcessOptions::cache` is `false` by default here, inverting the library's default, because the
library's default is right for an application and wrong for a measurement: with the cache on,
`process` encodes the result and writes it inside the call being timed. For a 4x Kyoto of the sample
that is a 2560x2560 PNG per run, which the reference measured at roughly 15% of the reported figure.

`--cache` turns it back on to measure what `process` costs an ordinary caller, and then the runs have
to be stopped from serving each other. The reference mutates `input.Hash` per iteration and restores
it in a `defer`, with a comment about how an early return would otherwise leak a mutated hash into
every model that follows. Here `Picture::new(path, pixels, identity)` takes the identity as an
argument and the pixel buffer is an `Arc`, so each run constructs its own `Picture` over the same
`Arc` with an identity of its own — `<base>-<model>-<nanos>-<n>` — and there is nothing to restore
and nothing to leak. The pixels are identical, so every run does identical work; only the key
differs.

Done unconditionally, not only under `--cache`: it costs an `Arc::clone` and a `format!` per run, and
it means the cached and uncached paths measure the same thing.

### D5. The loop, and what owns a session while it runs

Per model, in order: **release every resident session → warm-up runs (untimed) → release again →
cold start (timed) → timed runs → statistics**.

The two releases are different jobs. The first is the boundary between models: it stops one model's
sessions from holding memory while the next is measured, and it is what makes a downgrade
attributable to the model that hit it. The second is what makes the cold start a cold start — it
drops the session the warm-up built, so the next call pays graph optimization plus the provider's own
compilation (the cuDNN algo search on CUDA, an engine build on TensorRT, an MLProgram compile on
CoreML).

**Ownership and lifetime, which is why this is enough.** `chain::run` accumulates every
`SessionHandle` it acquires into a vector it holds for the length of the call, so a session a run is
using cannot be reclaimed underneath it. Those handles drop when `process` returns. The sweep is
sequential and single-threaded across models, so at the point either release is called **nothing is
outstanding**: the cache's own `Arc<Resident<S>>` is the last reference, the entry reaches refcount
zero inside `clear()`, and the native session is dropped there, synchronously, before the call
returns. That is precisely the guarantee slice 5 wrote into the `model-sessions` spec, and it is what
the reference has to implement by hand — its `CleanRegistry` *waits* for the models it removed to be
destroyed, with a comment saying this is what stops a cold run from quietly reusing a resident
session. Here there is nothing to wait for.

Two consequences the harness must respect, or the guarantee is void:

- **Models are measured one at a time.** A concurrent sweep would have another model's handles
  outstanding at the release point, so a "cold" start could find a session still resident. It would
  also be measuring contention rather than the model. Sequential is a correctness property here, not
  a simplification.
- **The previous model's `Enhanced` is dropped before the next model starts.** It holds an
  `Arc<DynamicImage>` — 19 MB for one 2560x2560 result, 150 MB at 8x 16-bit — and holding the whole
  sweep's outputs would change what the later models are measured under. Only the dimensions and the
  provider report are kept.

A background sweep of unused sessions also exists in the library (`sessions`' idle reclamation). It
is not something this loop can rely on or needs to disable: an explicit release is stronger and
immediate, and a sweep that fired mid-measurement would only remove what the run is not holding.

### D6. Nothing draws while a measurement is running

No progress callback is registered on a timed run. `ProcessOptions::on_progress` defaults to `None`
and stays `None` here, for the reason the reference states plainly: the models invoke it per tile,
and anything that writes to a terminal from inside the timed section is inside the measurement.

Install progress *is* reported, but only where it cannot touch a timing: `InitOptions::on_progress`
during startup, for the ONNX Runtime and GPU library downloads, written to **stderr**. Without it a
first run looks hung for several minutes. A model's own weights are downloaded during the warm-up,
which is untimed by construction, so a progress line there is safe — and this is exactly the seam
slice 7 attaches its viewport to.

The split of streams is fixed here because `--json` depends on it: the report — rendered or JSON —
is the whole of stdout, and progress, warnings about the configuration and any diagnostic go to
stderr. `perftest --json > results.json` must produce a parseable file on a machine that downloaded
a runtime on the way.

### D7. The provider report is folded per model across every run it took

`Enhanced::providers` carries `requested` beside the ordered set of providers sessions were actually
built on. The harness unions that set across the model's cold start and its timed runs, and reports
the model as downgraded where the union contains anything other than what was requested.

Union rather than "the cold start's report", although the cold start is where the session is built
and so where a downgrade is decided: the sessions the timed runs use are the cold start's, so the two
agree in every case anyone has described — and a union cannot be the thing that misses one, at the
cost of an insert per run into a set of at most five values. Where the requested provider is `Auto`,
"downgraded" is not a meaningful judgement, and the report instead states what it resolved to.

A run reporting an **empty** set — nothing was executed — is reported as such and never as "ran on
what was asked for". With `--cache` on and a bug in D4's identities that is exactly what a fully
served-from-store measurement would look like, so this is also the harness's own check on itself.

### D8. Two renderings over one sweep result, and the sweep never knows which

The sweep produces a value — the run's configuration, and per model an outcome that is one of
completed, refused, failed or interrupted — and returns it. Rendering happens afterwards, twice over:
a plain report, or a JSON document under `--json`.

This is the seam slice 7 needs. A live view is a third consumer of the same progression of outcomes,
and shaping the sweep around a renderer now is what would make it a rewrite then. The sweep therefore
takes a listener it notifies as each model starts and finishes, which in this slice is either silent
or writes one line per model to stderr, and in slice 7 is the viewport.

**The plain report** is the reference's, structurally: a header (D9), the summary table, the
failures, the refusals, the downgrade warnings, and a footer naming the two things a reader must not
assume — that the throughput figure is over the *input* megapixels, and that nothing here controls
for GC, CPU pinning or thermal throttling.

**The JSON document** is one object: a schema version, the tool's version, the configuration the
sweep ran under, and an array of per-model results each carrying its outcome and, where it completed,
its durations in milliseconds and its statistics. A schema version because the first consumer of this
will be a script somebody writes six months from now against a shape that has since moved; one
document rather than a stream of lines because the header is not per model and a comparison needs it.
Durations as numbers in milliseconds, not as the formatted strings the table shows — the table's
`371.3ms` is for reading.

### D9. The header is part of the contract, not decoration

The `performance-benchmark` spec requires the report to state the conditions. Concretely: the image
and its dimensions and megapixels; the requested provider beside what this machine supports, read
from `Opai::providers()`; the precision; the timed and warm-up counts, with the warning where the
warm-up is zero that the cold start may include a download; the parameter values; whether the cache
is on; where the log file is; and how many models were selected.

Two lines are this project's own rather than the reference's. **The log path**, because
`logging::init` returns it and the file is where the per-tile and per-session records went — which
replaces the reference's `-v`-prints-the-debug-log entirely (D10). **The cache mode**, from
`Opai::cache_mode()`, which reports whether the store behind the library is `Disk`, `Memory` or
`None` — a different fact from whether this run used it, and one that changes what `--cache`
measures.

The reference's budget line is omitted, per the proposal: `ModelMemoryStats` has no counterpart here.

### D10. `--verbose` prints the runs, and says where the log is

The reference's `-v` installs a `slog` debug handler on stderr and prints every timed run in order.
Half of that has no counterpart: `opai::logging::init` is latched to a single file sink, and there is
no public way to add a stderr subscriber. That is the logging slice's decision and this slice does
not reopen it.

So `--verbose` here prints every timed run in order — which is how a thermal ramp that the median
hides becomes visible — and the header's log path covers the rest. `RUST_LOG` still reaches the
file's filter, which is the documented way to turn the library's own records up, and the crate's
README says so.

### D11. The shared configuration directory, the identity `perf`, and what that costs

`Opai::initialize(APP_NAME, InitOptions { app: Some(PERF.into()), models, on_progress })` — the same
directory the GUI and the CLI use. The user's decision, and the reasons are worth recording: a sweep
reuses the installed runtime and the installed weights instead of pulling ~175 MB and several
gigabytes into a directory of its own, and `--skip-verify` only means anything if the directory the
operator drops a re-exported `.onnx` into is the one the harness reads.

Two costs, both stated rather than worked around:

- **It is refused while the GUI is running**, by the claim slice 1 takes inside `initialize`. The
  harness catches `InitError::AlreadyRunning`, renders the holder — *"gui, pid 4821"* — and exits
  with a message saying to close it. It does not retry, and it does not offer a flag to bypass the
  claim: everything the claim protects is exactly what a sweep touches.
- **`--cache` writes into the image cache the GUI shares**, evicting whatever it had stored. Off by
  default, so the default sweep writes nothing, and the README says what turning it on costs.

`perftest list` and `--help` are before all of this, and contend with nothing — which is what the
initialization path's own comment already promised: *"`--version`, `--help` and a catalogue listing
all still work while another process is running."* This slice is the first thing that makes that
promise load-bearing.

### D12. `--skip-verify` is passed at initialization and is unreachable elsewhere

It maps to `InitOptions::models = ModelTrust::LocalFiles`, which is the only place the library
accepts it — there is no setter and no per-run field, deliberately, per slice 5's D3. The harness
prints a warning line when it is on, because a sweep of weights nobody verified is the first thing to
suspect in a surprising number; the library writes its own `warn` record per acquisition into the log
as well.

Nothing about it reaches a downloaded file: a model whose files are not all present is installed and
verified under the ordinary rules whatever this flag says. That is the divergence slice 5 recorded
and this slice inherits without widening.

### D13. The sample is embedded, and a photograph of your own can be measured instead

The workspace root's `fixtures/test.dat` — the 640x640 JPEG carried over from the reference so a
measurement here can be compared with one taken there — is embedded into the binary with
`include_bytes!`, materialized to a temp file and loaded through `opai::image::load`. Embedded
rather than read from `CARGO_MANIFEST_DIR` at run time, which is what `inference/live.rs` does and
which is right for a test and wrong for a binary someone copies out of `target/release`.

**The root fixture, not a copy of it under `crates/perf/`.** That directory's own README states what
it is for — *"files shared by more than one crate's tests, which is why they live at the workspace
root"* — and the live inference checks already read this one. A second copy would be a second thing
to keep in step with the reference, and the whole value of the file is that it is the same bytes the
Go application measures. `include_bytes!` resolves relative to the source file, so the path is
`../../../fixtures/test.dat` and the bytes are still in the binary: sharing the file costs nothing
that embedding it bought.

The temp file is `rust-sak`'s `fs` RAII temp handle, which is the module the project context names
for exactly this and which is what the reference reaches for too (`go-sak`'s `MkTempFile`). It is a
temp *file* rather than an in-memory decode because `opai::image::load` takes a path, and loading
through it is also what checks that a file the loader produced is a file the pipeline accepts.

`--image <path>` measures a photograph of your own instead — the same idea as the live check's
`OPAI_LIVE_PHOTO`, and the answer to the sample being 0.41 megapixels when the images this
application exists for are twenty times that. The header reports whichever was used with its
dimensions, so the two cannot be confused in a pasted table.

### D14. The statistics are eighteen lines here, not a dependency

Minimum, maximum, mean, median and sample standard deviation over at most a few dozen durations,
computed over `f64` nanoseconds and rounded once at the end so the mean does not inherit an integer
division's truncation. Bessel-corrected (`n-1`), because the runs are a sample of the runs the
machine could have produced — at the default of five the population formula understates the spread by
about 11% — and zero for a single run.

`criterion` is the obvious crate and is the wrong tool: it owns the loop, decides the sample size
itself, and is built for microbenchmarks of a function called thousands of times, whereas one
iteration here is a multi-second inference that downloads gigabytes the first time. `statrs` would
supply the five functions and nothing else. Neither is worth a dependency for arithmetic whose entire
content is the Bessel correction, which has to be written down and argued for wherever it lives.

## Crates, dependencies and validation

**Crate split.** `crates/perf` is new and is where all of this change lands: the argument surface,
the selection, the sweep loop, the statistics and the two renderings. `opai`, `gui` and `cli` are
untouched — nothing here is logic a front end would need, so nothing here passes the project's test
for belonging in the library, and D2 is the decision not to put anything there on a benchmark's
account.

**`rust-sak`.** One feature, `fs`, for the RAII temp file D13 materializes the embedded sample
through — the module the project context names for temp handles, reached for instead of `tempfile`
for that reason. Nothing else here is in its territory: it has no module for argument parsing,
terminal tables or statistics, and none of the three is a gap worth extending it with. A summary
table is presentation for one binary rather than infrastructure another project would ask a
first-party utility crate for; `clap` is a solved problem with an ecosystem standard; and the
statistics are eighteen lines (D14). The image is decoded through `opai::image::load`, which is
`rust-sak`'s `image` module already, rather than by depending on it a second time here.

**Other crates, each weighed:**

- **`clap` 4 (derive)** — argument parsing, and the crate the series' plan named at slice 5. It is
  the ecosystem standard (4.6.6, August 2026); the derive API gives the flags, the validators, the
  `list` subcommand and `--help` from the struct that already has to exist. `argh` and `bpaf` are
  smaller and neither is what a Rust developer expects to find in a repository. The provider flag
  parses through the library's own `FromStr for ExecutionProvider`, so the spellings this accepts
  cannot drift from the ones the library publishes; `Precision` has no `FromStr`, so its three
  spellings are a `ValueEnum` mapped onto the library's values in one place.
- **`comfy-table` 8** — the summary table. Actively maintained (8.0.0, August 2026), minimal, one
  `unsafe` in the whole dependency tree and that inside the TTY probe, which is a feature that can be
  turned off — and this slice turns it off, because TTY detection is slice 7's and a table crate is
  not where it should arrive. `tabled` is derive-first and larger for a single static table;
  `cli-table` is smaller but much less used; `prettytable-rs` is unmaintained with an advisory
  requested against it. Hand-rolling column widths was weighed and rejected on the wrapping: the
  reason column carries a library error message of unpredictable length, and content-aware wrapping
  is the whole of what this crate is for. The reference uses `lipgloss`'s table, which has no Rust
  counterpart; `ratatui` (slice 7) renders frames of a live view, not a table into scrollback.
- **`serde` + `serde_json`** — `--json`. `serde_json` is the standard and is already in `opai`'s
  graph. No alternative was seriously weighed; hand-writing JSON is how a report ends up unparseable
  on the first error message containing a quote.
- **`tokio`** — `Opai::initialize` and `Opai::process` are `async`, so `main` needs a runtime, with
  `macros` and `rt-multi-thread` for it and `signal` for D-Ctrl-C. Already this workspace's runtime;
  `smol` or `async-std` would be a second one.
- **`thiserror`** — the project's error convention, matching `rust-sak`, `opai` and `gui`, and what
  the three error types here derive: the selection's, the input's and the unknown-parameter one. Not
  weighed against an alternative, because there is nothing to weigh: a fourth crate's errors spelled
  differently from the other three is the cost, and `anyhow` is the wrong shape for errors a caller
  matches on to decide an exit status.
- **`tracing-subscriber`** is *not* taken. The log sink is `opai::logging::init`'s, and installing a
  second subscriber is the thing D10 declines to do.

**Ctrl-C.** `tokio::signal::ctrl_c` cancels a `CancellationToken` passed as `ProcessOptions::cancel`,
which the library already honours — a run fails as a whole with `InferenceError::Cancelled` and
produces no partial image. The sweep breaks out, the model in flight is marked interrupted rather
than failed, the summary for the models that finished is printed, and the exit status is 1. Nothing
calls `std::process::exit` from inside the sweep, so the summary is never the thing that gets skipped.

**Platforms and execution providers.** Everything that is not a real inference is unit-tested beside
the code on every CI platform, with no runtime, no GPU, no model file and no network: the selection
(an unknown name, a duplicate name, and a precision a variant does not publish), the dropping of a
declined model against a constructed refusal, the statistics against hand-computed values, the duration
formatting, the JSON document's shape, the outcome-to-exit-status mapping and the downgrade fold of
D7 over constructed `ProviderReport`s.

What that does not cover is a sweep. A real one is run by hand, and this slice will be run on
**macOS arm64 against CoreML and the CPU**, which is the hardware this project has — the same two
providers `inference/live.rs` was run against and the same pinned ONNX Runtime 1.26.0. **CUDA and
TensorRT remain untested here, on any machine, as they have been in every slice of this project so
far**, and the downgrade reporting of D7 is therefore exercised against constructed reports rather
than against a real failed session build. Linux and Windows are untested for the sweep itself; the
parts that are unit-tested run on CI's runners for all three.

## Risks / Trade-offs

**The default sweep measures one model, and will look broken** → It is one model because one model
runs. Mitigated by the report saying how many were skipped and that naming one shows why, by the
header stating how many were selected, and by an explicitly named model still reporting its reason.
The sweep covers more of the catalogue as the model series lands, with nothing here edited.

**A declined model is discovered per sweep rather than known in advance** → The cost is one `process`
call per declined model per sweep, which installs nothing, opens nothing and returns before any
transfer — microseconds against a sweep measured in minutes. It is the price of D2, and it falls to
zero on its own as pipelines land rather than needing to be removed.

**A harness written against a surface no front end has used yet may be the wrong shape for the front
end that comes later** → Accepted, and it is partly the point: this slice is where the library's
ergonomics get their first real test. What limits the cost is that everything in `crates/perf` is a
consumer — nothing else depends on it, so a later front end that wants a different shape changes the
library and this crate follows. What this slice deliberately does *not* do is widen the library
itself on its own account (D2): a gap the harness feels is reported, not patched here.

**The sweep's own correctness cannot be tested automatically** → The loop needs a real runtime and a
real model, so the property that matters most — that a cold start is genuinely cold — is checked by
running it and reading the numbers, exactly as the reference's `bench_test.go` cannot check it
either. Mitigated by the one assertion that *is* mechanical: a run reporting an empty provider set
means nothing executed, which is what a broken identity in D4 would produce, and the harness reports
that rather than a timing.

**`--cache` churns the cache the GUI shares** → Off by default, stated in the header when on, and
documented in the README. Not worked around with a private cache directory, because measuring what
the shared cache costs is the only reason the flag exists.

**A sweep is refused while the GUI is running, and that will surprise someone** → It is the
single-instance guarantee working. Mitigated by a message that names the holder and its pid rather
than reporting a generic failure, and by `list` and `--help` working regardless.

**`comfy-table`'s major version is recent** → 8.0.0 is two months old at the time of writing. Its
exposure is one module: the summary table is built in one function from rows of strings the rest of
the crate produces, so replacing it is that function and nothing that measures anything.
