## Context

See proposal.md - Why. What matters here is the shape of the code this lands in.

`Opai::initialize` resolves the configuration directory, takes the single-instance claim, then calls
`Self::select(gpu::adapters(), OS, ARCH)` to build a private `Plan { runtime, gpu }` and hands it to
`Self::install`. `install` holds an `install_one` closure - dependency-only - which resolves the
directory, builds one `Reporter` per dependency with `Reporter::for_dependency`, and calls
`deps::install::install`. The models path reaches the same `deps::install::install` from
`deps/model/mod.rs`, with a descriptor built elsewhere.

Inside `install_inner`, the steady-state check computes `(previous, current)` on a blocking thread
and returns `Ok(())` early when `current` holds. That check is the one constraint the whole design
had to bend around: its own comment records that it is *"neither cheap nor rare - `intact` is one
`symlink_metadata` per recorded file, a CUDA or TensorRT tree is thousands of them, and this runs for
every dependency on every launch"*.

`Plan` already carries everything the plan report needs. Each `Descriptor` holds `progress: Which` -
the public `Dependency` enum - and `sources: Vec<Source>`, whose sizes `Reporter::for_dependency`
already sums. Nothing has to be computed that is not computed today; it has to be said out loud.

The design authority for what consumes this is screens 02 and 02b in the Claude Design project,
whose component list shows every dependency the machine needs with a per-row state and size. They are
deliberate redesigns rather than ports - the Wails dialog renders `Object.entries(downloads)` and so
can only show a dependency that has already reported.

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | done |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs | 1 | done |
| 3 | `add-gui-3-init-plan` | `InitOptions::on_plan` and the already-installed report, so the install list, its per-dependency sizes and each dependency's outcome are observable before and during a first run. `opai` only | - | **CURRENT** |
| 4 | `add-gui-4-setup-progress` | Screen 02 - `prepare_library_path` at the top of `main`, the `initialize` command, progress and plan events, the managed `Opai` handle, the setup dialog, automatic start, Quit | 2, 3 | pending |
| 5 | `add-gui-5-setup-error` | Screen 02b - the failure dialog, per-dependency outcome, Try again and Quit | 4 | pending |

**Slice 3 is this change.** The remaining 19 designed screens are not yet cut into slices; they are
cut when they are reached, and the table carries only what has been committed to.

**Crates.** `opai` only: `src/lib.rs`, `src/progress.rs`, `src/deps/install.rs` and the one call site
in `src/deps/model/mod.rs` that has to keep compiling. `gui` and `cli` are untouched - slice 4 is the
first consumer.

Nothing in this slice could live in a front end. Which dependencies this machine needs is the GPU
probe's conclusion; the sizes are the pinned table's; whether one is already installed is the install
record's. A front end answering any of them would be re-probing the hardware and reimplementing the
fingerprint-and-`intact` rule, and `cli` would need the same thing on its own first run.

**rust-sak: `fetch`, `crypto`, `fs` and `sysinfo` are all already in use on this path and none of
them gains or loses anything here.** `fetch` owns the transfer, the retries, the resume and the live
progress this reports *over*; `sysinfo` supplies the GPU probe behind `Plan::select`. What this change
adds is `opai`'s statement about its own dependency set, which is not general-purpose infrastructure
and is not a candidate for extending `rust-sak`.

**New crates: none.**

## Goals / Non-Goals

**Goals:**

- A front end can draw every row of screen 02 - including the queued ones and the already-green ones -
  before the first byte moves.
- The report costs nothing on the path that does not ask for it, and costs no extra filesystem work
  on the path that does.
- The model install path's observable behaviour is unchanged, so `model-install`'s spec needs no
  delta in this slice.

**Non-Goals:**

- **An overall percentage.** Composing one figure across dependencies is presentation: the weighting,
  the rounding and the "1 of 3 components installed" sentence all belong to whoever draws the dialog,
  and the `initialization` spec already refuses to emit a single combined fraction for reasons that
  have not changed.
- **A pre-flight query.** Nothing here answers "what would you install?" outside an initialization -
  see the decision below.
- **Naming the dependency on `InitError`.** See the decision below.
- **Reporting a dependency's outcome for models.** The `model-install` spec says an install that is
  already current reports nothing, no consumer wants otherwise, and widening it here would be a spec
  change with no caller.

## Decisions

### The plan is a callback on `InitOptions`, not a separate query

`InitOptions` gains `on_plan: Option<OnPlan>`, where `OnPlan` is
`Arc<dyn Fn(&[PlannedDependency]) + Send + Sync>` - the same `Arc<dyn Fn>` shape `OnProgress` already
has, for the same reason: it crosses thread boundaries and a generic would not survive being handed
to a closure that already is one. It is fired once from `Opai::initialize`, between `Self::select`
and `Self::install`.

`PlannedDependency` is the public row: the `Dependency` it names and the `u64` sum of its sources'
published sizes. `u64` rather than `Option<u64>` because a pinned release archive always has a
published size - the `Option` on `Progress::total` exists for a response that declared none, which
cannot arise before a request has been made.

Firing between `select` and `install` is what makes the ordering a property of the code rather than a
convention: the claim is taken, the hardware is probed and the plan exists, and `install` - the first
thing that creates or transfers anything - has not been called. The refused-initialization scenario
in the spec falls out of the same placement, since `AlreadyRunning` is raised above it.

*Alternatives considered.* **A pre-flight `Opai::plan(name)`**, called by the front end before
`initialize`. Rejected for two reasons: it probes the GPU a second time, and it is a second answer to
a question `initialize` answers again a moment later - the list shown could differ from the list run,
and nothing would detect it. It would also have to decide what to do about the claim, which
`initialize` takes and a query has no business taking. **Widening `on_progress` to carry a "planned"
report per dependency** - no new callback and no new type. Rejected because it makes a recipient
reconstruct a list from a stream of single rows and guess when the list is complete; the plan is one
value and is honest as one.

### The already-installed report is an outcome from `install`, not a branch inside it

`deps::install::install` changes its return type from `Result<(), InitError>` to an outcome -
`Installed` or `AlreadyCurrent` - decided at the existing early return. `install_one` in
`Opai::install` acts on it and emits the one terminal report through the `Reporter` it already holds;
`deps/model/mod.rs` discards it and reports nothing, exactly as today.

This puts the knowledge where the difference actually is. Whether a skipped install is worth
reporting is not a fact about installing - it is a fact about *what is being installed*, and the two
callers are the only things that know which they have. `install_inner` stays branch-free on this
question, which matters because its one existing branch (`Contents::Archive`) is explicitly
documented as the only one.

*Alternatives considered.* **Emitting from inside `install_inner`, gated on
`matches!(descriptor.progress, Which::Model(_))`.** The discriminator exists, so this works - but it
is a second place that knows the difference between a model and a dependency, and it puts a
front-end-facing policy decision inside the shared pipeline. **Emitting for models too**, and
amending the `model-install` spec. Rejected as scope: no consumer wants it, and it would put a spec
change with no caller into a slice that exists to unblock a dialog.

### It is a `Phase` variant, and the name is doing work

The report is a `Progress` carrying a new `Phase::AlreadyInstalled`, `bytes` of 0, `total` of the
published size and `fraction` of exactly `1.0`. `Phase` is `#[non_exhaustive]`, so the variant is
additive for any external matcher.

The tension is worth naming rather than discovering later: `Phase` is documented as "which part of an
install a report is about", and an install that did not happen has no parts. The variant is
deliberately *not* called `Skipped` or `Done`, because a recipient has to be able to tell it from a
transfer that finished - both carry `fraction: 1.0`, and only the phase distinguishes "this is on
disk and always was" from "this just finished downloading". `AlreadyInstalled` says which, at the one
place a front end reads.

`bytes: 0` rather than the published size, with `total` carrying that size: the field means "bytes
moved in the current phase" and nothing moved. A front end wanting the size has it on the plan row
and on `total`.

*Alternative considered.* **A separate `on_skipped` callback.** Rejected because it splits one
dependency's story across two channels, and a front end would then have to merge two streams to
decide a single row's state.

### `InitError` is untouched, and the front end attributes the failure

Screen 02b names the dependency that failed and how far it got. Neither needs the error to say so.
Installs run strictly in order, the plan gives that order, and every dependency reports either a
terminal `1.0` or its last fraction - so the failing dependency is the first one in the plan without
a terminal report, and "Stopped at 9% · 37 MB of 412 MB" is the last `Progress` the recipient held.

Adding a `Dependency` to `Download`, `HashMismatch`, `Fs` and `Io` would be a second answer to a
question the stream already answers, and the two could disagree - an error raised from the
bookkeeping after a successful transfer would name a dependency the stream had already reported as
finished.

This is also what keeps slice 5 honest about the failures that are *not* attributable to any
dependency at all. `RuntimeLoad`, `RuntimeStart` and `RuntimeUnavailable` are raised after every
dependency is on disk, so every row is green and the failure belongs to the dialog's own message
rather than to a row - which is a property of where they are raised, not something a field on the
error could fix.

### Reported for every dependency, including the ones already there

The plan lists what the machine *needs*, not what is *missing*. A list of only the missing ones has
no denominator for "1 of 3 components installed", and would be a different length on the second
launch than on the first - so the dialog would change shape between two runs of the same build on the
same machine for reasons a user cannot see.

The consequence is that the plan cannot say which rows are already installed, because that is exactly
the check this design refuses to hoist - see below. A recipient learns it from the outcome reports
that follow, within milliseconds on a steady-state launch.

### The steady-state check is not hoisted, and the plan does not carry "installed"

The rejected design had the plan carry `installed: bool` per row, which would have meant running
`manifest::read` and `Manifest::intact` for every dependency before the install loop - and then again
inside it, or restructuring `install_inner` so the loop consumed a pre-computed result.

Running it twice doubles the most expensive part of a launch that has nothing to do; that is the
comment at `install.rs`'s own steady-state check, which says so in those words. Restructuring to
compute it once means threading `(previous, current)` from a pre-pass into `install_inner`, whose
trusted-model path deliberately computes `want` differently and skips `intact` altogether - so the
pre-pass would need `ModelTrust` and a branch, in a function that today has neither.

Neither cost buys anything the sequencing does not already give. The dialog opens on the first
`Downloading` report, by which point every dependency ahead of it in the plan has already reported
its outcome - so the rows a user actually sees are already correct, and a row is only ever briefly
"queued" before turning green. On a machine with nothing to install there is no dialog at all.

*This is a reversal of the first design and it is recorded deliberately*, because the hoist is the
obvious thing to reach for again and the reason not to is a performance property that no test will
report.

## Risks / Trade-offs

- **A row can flicker from "queued" to "installed".** On a launch where a later dependency needs work
  and an earlier one does not, the earlier row is drawn queued and turns green almost immediately. →
  Accepted, and bounded by the dialog opening on the first `Downloading` report rather than on the
  plan: by then every earlier outcome has been reported, so the flicker is not on screen. The one
  case it could be seen is a dependency whose transfer starts before an earlier one's `intact` check
  finishes, which cannot happen - the installs are sequential.
- **`Phase::AlreadyInstalled` is a phase that is not a phase.** A future reader may take it for a
  fourth kind of work. → Mitigated by the doc comment stating what it is and why it is not `Skipped`,
  and by the spec requirement that it carries no transferred bytes.
- **A `#[non_exhaustive]` enum gaining a variant is source-compatible and not behaviour-compatible.**
  An external matcher's `_` arm will now catch a report it has never seen, and a front end that
  counts reports per dependency will see one more than before. → No external consumer exists yet;
  `gui` and `cli` neither register `on_progress` nor match on `Phase` today, and `perf` is in-tree.
- **Two reports could be taken for two dependencies.** A recipient keying state by `Dependency` is
  fine; one appending rows per report would draw a dependency twice. → The spec pins the report as
  exactly one per skipped dependency, and slice 4's store is keyed by dependency.
- **The plan is only as honest as the probe.** A machine whose adapters cannot be enumerated is
  treated as having no NVIDIA adapter, so the plan names the runtime alone - and a user with an
  NVIDIA card sees a one-row dialog with no explanation. → Pre-existing behaviour, unchanged by this
  slice: the same probe already decided what got installed. It is worth recording as a parity note
  for whoever builds the settings screen, not a defect this change introduces.

**Platforms.** The logic is platform-independent and every path is exercised by the existing suite's
fake-server installs, which run on all three targets in CI. What differs per platform is only the
*length* of the plan - one row on macOS, up to four on Windows and Linux with NVIDIA hardware - and
the multi-row case cannot be exercised on the developer's macOS machine, so it is covered by tests
driving `Plan::select` with synthetic adapter lists rather than by a real RTX run. **No execution
provider is involved:** this change runs no model, builds no session and loads no runtime - it
reports on the transfers that happen before any of that.
