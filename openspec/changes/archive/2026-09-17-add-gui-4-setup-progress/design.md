## Context

See proposal.md - Why. What matters here is what the two sides already are.

`crates/gui/src/lib.rs` is a `run()` that installs the log sink, registers the single-instance plugin and the
OS plugin, and hands `generate_handler![version]` to `tauri::Builder`. Its two ordering comments are the
constraints this change has to fit between: the log sink goes **first** because it takes no claim and refuses
nobody, and the single-instance plugin goes first among things that do *work*, because a second launch leaves
from inside its setup hook via `std::process::exit(0)`. That second comment already names this change:
*"a future `Opai::initialize` must stay AFTER the plugin has had its say… `Builder::build()` is the boundary,
so `initialize` called from the app's own `setup` hook or from a command is on the right side of it; called
from `main()` before the builder, it is on the wrong side."*

On the library side, slice 3 finished the reporting surface. `InitOptions` carries `on_plan` and
`on_progress`, both `Arc<dyn Fn>`; the plan is one `&[PlannedDependency]` fired between `Self::select` and
`Self::install`; every dependency reports either a running fraction or one terminal `Phase::AlreadyInstalled`
at `fraction: 1.0` with `bytes: 0`. None of those types derives `serde`, and `Dependency` and `Phase` are both
`#[non_exhaustive]`.

`prepare_library_path` has no caller in the workspace - `cli` prints a version and `perf` starts straight into
`Opai::initialize` - so the GUI is the first binary to honour its contract: the first statement of `main`,
before any other thread, before the log sink, and on Linux it does not return.

The design authority for what this draws is screen 02 in the Claude Design project: a modal over the shell,
an overall bar with "1 of 3 components installed", one row per component with its name, its size and its
state, the GitHub throttling note, and Quit in the footer.

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | done |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs | 1 | done |
| 3 | `add-gui-3-init-plan` | `InitOptions::on_plan` and the already-installed report, so the install list, its per-dependency sizes and each dependency's outcome are observable before and during a first run. `opai` only | - | done |
| 4 | `add-gui-4-setup-progress` | Screen 02 - `prepare_library_path` at the top of `main`, the `initialize` command, progress and plan events, the managed `Opai` handle, the setup dialog, automatic start, Quit | 2, 3 | **CURRENT** |
| 5 | `add-gui-5-setup-error` | Screen 02b - the failure dialog, per-dependency outcome, Try again and Quit | 4 | pending |

**Slice 4 is this change.** The remaining 19 designed screens are not yet cut into slices; they are cut when
they are reached, and the table carries only what has been committed to.

**Crates.** `gui` only: `src/main.rs`, `src/lib.rs`, a new `src/setup.rs`, and the frontend under
`crates/gui/frontend/` - one IPC module, one store, one dialog feature, two generated shadcn primitives, the
i18n keys and the tests. `opai` and `cli` are untouched.

The one thing in this slice that could have been argued into `opai` is the overall figure, and slice 3 already
refused it there: composing several dependencies of wildly different sizes into one number is a weighting
decision, and "1 of 3 components installed" is a sentence that has to pluralise in thirteen languages. Both
belong to whoever draws the dialog. Everything else here is a window, an IPC boundary and a process's startup
order. `cli` could consume none of it - `perf`'s `install_progress` already shows what the same two callbacks
look like rendered as lines of text.

**rust-sak: nothing is used directly and nothing is missing.** The transfer, the retries, the resume, the
hashing and the hardened extraction are `fetch`, `crypto` and `fs`, already in use *inside* `opai` on the path
this change reports over; `sysinfo` is behind the GPU probe that decides the plan's length. This change moves
no bytes and verifies nothing - it relays reports and draws them - so there is no infrastructure here for
`rust-sak` to already provide and no gap worth extending it with.

**New crates: none.** `serde` with `derive`, `thiserror` and `tauri` are already `gui` dependencies; the
serialized command's mutex is `tauri::async_runtime::Mutex`, tokio's re-exported through Tauri, so a front end
gains no runtime crate of its own. On the frontend, Dialog and Progress are shadcn *generated source* over the
`radix-ui` package slice 1 already installed, so no npm dependency is added either.

## Goals / Non-Goals

**Goals:**

- A first launch installs itself, says what it is installing and how far along it is, and needs nothing from
  the user but patience or Quit.
- A launch with nothing to install is indistinguishable from one with no setup step at all - no dialog, no
  flash of one.
- The startup order is a property of the code rather than of a comment: nothing can run before the library
  search path is established, and nothing can initialize before the single-instance plugin has had its say.
- The `Opai` the initialization produced is where every later slice can reach it.
- Two initializations of one process cannot overlap, whatever asks for them.

**Non-Goals:**

- **A failure dialog.** Slice 5. This slice specifies what a failure leaves on screen and no more.
- **Showing the providers the initialization decided.** `Opai::providers` is what a Settings screen reads, and
  there is no Settings screen. Returning it now would be a serialized type with no reader.
- **A cancel.** Quit is the only way out of an unfinished setup, which is what the design draws. Cancelling a
  transfer and staying in an application that cannot run anything is not a state worth building.
- **Reporting model installs.** `Dependency::Model` exists and reaches the same reporter, but no model is
  installed during initialization - the first one lands when an operation runs, which is a later slice's
  screen. The wire shape does not exclude it; nothing in this slice produces one.

## Decisions

### `prepare_library_path` is called in `main`, and its failure travels into `run`

`crates/gui/src/main.rs` becomes:

```rust
fn main() {
    // SAFETY: the first statement of main; no other thread exists yet.
    let prepared = unsafe { opai::prepare_library_path(opai::APP_NAME) }.map(|_| ());
    gui_lib::run(prepared)
}
```

Three things are decided there. **It is in `main`, not in `run`**, because the contract is the first statement
of `main` and `run` is not that - the log sink is installed in `run`'s first lines, and on Linux this call
replaces the process image, so a sink installed before it would leave the replacement holding a file handle
whose writer died with the old process. **The returned directories are discarded**, because nothing in `gui`
needs them: `Opai::initialize` resolves its own directories from the same application name, and holding a
second copy here would be a second answer to where the libraries are. **The failure is carried rather than
unwrapped**, and reported in `run` once the log sink exists - which is the only place it can be reported, since
there is no log yet when it happens.

A failure here is not fatal. It means a directory could not be created, which `Opai::initialize` will run into
again a moment later and report through the dialog; failing the launch instead would replace an explanation
with a window that never opens. This matches what the crate already does with a log sink that cannot be
installed, and what `prepare_library_path` itself does with a restart that fails.

*Alternative considered.* **`expect()` on the result.** Rejected: the one failure mode is an unwritable
configuration directory, and a panic before any window exists gives a user nothing to read.

### One `Setup` state that is the lock, the memo and the handle at once

`src/setup.rs` holds

```rust
pub(crate) struct Setup { ready: tauri::async_runtime::Mutex<Option<Opai>> }
```

registered with `.manage(Setup::default())` and taken by the command as `State<'_, Setup>`. The command locks
it for the whole of its work: if the option is `Some`, it returns immediately; otherwise it awaits
`Opai::initialize` **while still holding the lock**, and stores the handle on success.

That one field answers all three of the questions this slice has about initialization. It is the **lock**,
because the guard is held across the await, so a second request waits rather than starting a second install -
the Wails app's `initMu`, kept for the reason its comment gives, and the requests genuinely overlap here:
React's StrictMode double-mounts the start-up effect in development, and slice 5's Try again is a third call
site. It is the **memo**, because a stored handle is a success to return rather than an install to repeat, and
because it stores *only* success a failed attempt stays retryable. And it is the **handle** every later slice
reaches through `State<'_, Setup>`.

`tauri::async_runtime::Mutex` rather than `std::sync::Mutex`: the guard is held across an `.await`, which a
`std` guard cannot be, since it is not `Send`. It is tokio's, re-exported by Tauri, so this costs no
dependency.

*Alternatives considered.* **`app.manage(opai)` on success**, with a separate lock. Rejected: `manage` is
keyed by type and silently does nothing on a second call, so a handle replaced after a retry would be ignored
with no error - and it would still need the lock beside it, which is two pieces of state holding one fact.
**A `OnceCell<Opai>`.** Rejected: it has no way to express "failed, try again" - either it latches a failure or
it is not the thing that serializes, and the retryability is a requirement.

**Ownership and lifetime.** `Opai` is `Clone` over an `Arc<Inner>`, and `Inner` holds the single-instance
claim, the provider report, the session cache and the run cache. Storing it in managed state means the claim
is held for exactly as long as the application is up and released when the process exits, which is the
intended lifetime; nothing in this slice clones it out. A retry that succeeds replaces a `None`, never a
`Some`, so there is no window in which the claim is dropped and retaken. Two `Opai`s in one process share one
claim anyway, and the ONNX environment behind them is process-global and never torn down - both of which
`opai`'s own documentation states, and both of which are what make a retry-after-failure safe rather than
merely tolerated.

### The reports travel on one `Channel`, not on two global events

The command takes a `tauri::ipc::Channel<SetupEvent>` as an argument, and the frontend creates it and attaches
its handler before invoking:

```rust
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum SetupEvent {
    Plan { rows: Vec<PlanRow> },
    Progress(ProgressRow),
}
```

`on_plan` and `on_progress` are closures over a clone of that channel; each maps the library's report to the
wire type and sends it, logging and discarding a send error - the install must not fail because a window went
away.

A channel rather than `Emitter::emit` with two event names, which is the Wails shape (`app:download`) and the
obvious port. Three reasons. **Ordering**: one channel carries the plan and the progress reports in the order
they were produced; two event names are two streams whose relative order nothing guarantees, and a progress
report drawn before the plan that dimensions it is a row with no size and no denominator. **Lifecycle**: a
channel's handler is set before the `invoke` that hands it over, so no report can be emitted before there is
somewhere to put it - with global events, the plan fires within milliseconds of the command starting and is
lost to a listener that has not finished registering, which is an async gap in `listen()`'s own contract.
**Drift**: an event name is a string written once in Rust and once in TypeScript with nothing to notice a
rename, which is the hazard `frontend/ipc/app.ts` already carries a comment about for `version`; a channel
has no name on the wire at all.

The cost is that this channel belongs to one call. That is the right shape for this one - the dialog exists
for the duration of one initialization - but it is worth saying that a later slice wanting reports outside a
command's lifetime (a model installing during an enhancement) will need a global event, and should not take
this decision as the project's rule.

### A dependency is identified on the wire by the name the library gives it

`PlanRow` is `{ name: String, size: u64 }` and `ProgressRow` is
`{ name: String, state: RowState, fraction: f64 }`, where `name` is `Dependency::as_str()`. It is both the row
key and the label the dialog shows.

Nothing in `gui` matches on `Dependency`. That is the point: the enum is `#[non_exhaustive]` and gains a
variant every time a dependency family does, and a translation table here would be a second place that has to
learn each one - it would compile unchanged and quietly file the new dependency under a `_` arm. `as_str()` is
already documented as *"the dependency's name, for a front end with nothing better to show"*, it is unique per
variant, and for a model it is the artifact id, which is unique per model.

It is not translated, and that is parity rather than an oversight: "ONNX Runtime", "NVIDIA CUDA", "NVIDIA
cuDNN" and "NVIDIA TensorRT" are product names, the Wails GUI hardcoded exactly these four strings at its four
emit sites, and the design draws them in the monospace face precisely because they are identifiers.

`Phase` **is** matched, because the dialog has to tell an already-installed dependency from one that is
working, and that match needs a `_` arm for the same `#[non_exhaustive]` reason. The arm maps an unknown phase
to "working", which is the safe direction: a new phase is a new *kind of work*, and treating it as work shows
a row in progress rather than claiming something finished that did not.

### The row states are Installed, Downloading, Extracting and Queued

The design's component list draws three states - Installed, Downloading, Queued. This adds a fourth,
Extracting, shown while a dependency's archive is being expanded.

It is a deliberate addition rather than a drift from the design file, and slice 3 is why. `DOWNLOAD_SHARE`
exists because *"a couple of gigabytes of LZMA2 is a minute or more of single-threaded work with nothing
arriving over the network"*, and the whole purpose of giving the expansion the last fifth of the bar was to
make that minute legible as work rather than as a hang. A row still labelled "Downloading" while nothing is
downloading takes half of that back, in the one place where the label and the bar would disagree for a whole
minute. The bar itself, the row layout and the colours are unchanged.

`Queued` is the state of a row that has reported nothing yet - which is what the plan gives us and what slice
3 deliberately declined to refine, having refused to hoist the `intact` check that would say which rows are
already installed. A queued row is briefly wrong about a dependency that turns out to be installed, and the
dialog not opening until the first transfer is what keeps that off the screen.

### The overall figure is the size-weighted mean of the per-dependency fractions

`overall = Σ(sizeᵢ × fractionᵢ) / Σ(sizeᵢ)`, over every row in the plan, with a row that has reported nothing
contributing `0` and an already-installed row contributing its whole size. The count beside it is the number of
rows whose fraction has reached 1, out of the number of rows in the plan.

Both come from things already on hand: the plan gives every size before anything moves, and `Progress.fraction`
is documented to be monotonic per dependency and to reach exactly `1.0` only when that dependency is finished.
Nothing has to be measured, and the figure cannot exceed 1 or walk backwards unless one of its inputs does.

*Alternatives considered.* **Summing `Progress.bytes` against `Progress.total`.** Rejected: `bytes` means
"bytes moved in the current phase" and resets to 0 when the expansion starts, so the sum would fall by a
gigabyte mid-install. **The plain count of finished components as the bar.** Rejected: it is the figure the
label already carries, and as a bar it would sit at 0% through a 612 MB download and then jump a third -
which is the dishonest bar slice 3's phase split exists to avoid, rebuilt one layer up.

### The error crossing IPC carries a message and nothing else

The command returns `Result<(), SetupError>`, where `SetupError` is a `thiserror` enum with one variant
wrapping `InitError`'s rendered message. `InitError` is neither `Serialize` nor `Clone`, so something has to
cross the boundary in its place; the rendered message is the whole of what this slice needs, because this
slice draws no failure state beyond leaving the dialog where it is.

Slice 5 is what needs structure - it names the failing dependency, states the reason and calls out the known
cause - and the argument recorded in slice 3's design is that it gets the *dependency* from the progress
stream rather than from the error. What it may still want from the error is which *kind* of failure it was,
so that "retrying in a few minutes usually works" is said about a throttled download and not about a runtime
that will not load. Adding that taxonomy now would be a serialized enum with no reader, which is the thing
this project does not do; the variant is deliberately shaped so that slice 5 widens it rather than replaces
it.

### Quit is a command of this application's own

`#[tauri::command] fn quit(app: AppHandle) { app.exit(0); }`, rather than adding `tauri-plugin-process` for
its one `exit` binding. The plugin would bring a dependency, a permission set and a second way to end the
process, for a call this crate can make in one line. It is the same judgement `capabilities/default.json`
already records about not granting `os:default` for six reads the application never makes - and commands this
application registers itself need no capability entry at all, which is why `version` works today with
`core:default` alone.

Exit code 0, because quitting from the setup dialog is a decision rather than a crash - the same code the
single-instance plugin's refused second launch uses. `AppHandle::exit` runs the exit path, so the managed
`Setup` is dropped with everything else; the claim would be released by the process ending in any case.

### The dialog opens on the first report of work, and the store is where that is decided

`frontend/stores/setup.ts` holds the rows keyed by name, the status, and the two reducers the channel handler
calls. It is **not** persisted: a plan describes this launch on this machine, and a stale one restored from
`localStorage` would be a dialog describing the previous run.

The status moves to `running` on the first progress report whose state is not `installed` - not on the plan,
which is what makes the flicker slice 3 accepted unobservable rather than merely brief. It moves to `ready`
when the command resolves and to `failed` when it rejects; the dialog renders while the status is `running` or
`failed`, which is one condition rather than a separate "is the dialog open" flag that could disagree with it.

A store rather than `useState` in `App.tsx`: the reports arrive from outside React, two components read the
result - `App` decides whether the dialog exists, the dialog draws the rows - and slice 5 adds a third reader.
It matches what `stores/enhancements.ts` already is, minus the persistence.

### The dialog is a Radix dialog with its dismissals removed

`DialogContent` is rendered with `onPointerDownOutside`, `onEscapeKeyDown` and `onInteractOutside` prevented,
and without the close button shadcn's generated content includes. The requirement is that it cannot be
dismissed, and Radix's own defaults are all three of the ways it can be - so each is turned off explicitly at
the one place the dialog is built, rather than by hiding the button and hoping.

`shadcn add dialog progress` is the generator, followed by the four corrections this project applies after
every run: the rewritten `cn` import, arrow functions, named React imports, and removing the bogus `cn`
package the generator adds to `package.json`.

### The Rust side's rule is tested against a fake, not against an install

`Setup` is generic over what it holds - `Setup<T>`, with `type Ready = Setup<Opai>` in the command - so its
three properties can be driven by a test with a fake initializer and no network, no runtime and no
configuration directory: that concurrent requests produce exactly one initialization, that a request after
success does no work, and that a request after a failure runs again.

This is the seam the crate already uses one layer down - `inference::process::Backend` is a trait *"so that
everything above it… is exercised on every CI platform against a fake"* - applied to the one piece of logic in
this slice that is a rule rather than a wiring. The wiring itself (which command is registered, what the
channel carries) is covered by the existing style of contract test: the frontend half asserts the command name
that goes on the wire, and the Rust half is pinned by `generate_handler![]` refusing to compile against a
function that does not exist.

## Risks / Trade-offs

- **`Opai::initialize` needs a tokio runtime and gets one by assumption.** It uses `spawn_blocking` internally,
  and async commands are spawned onto `tauri::async_runtime`, which is tokio. → Accepted, and it is not a
  configurable choice in Tauri 2; if that ever changed, the symptom is a panic on the first install rather
  than anything subtle. The same assumption is what lets this crate use `async_runtime::Mutex` instead of
  depending on tokio.
- **The window can be interacted with while the dialog is not yet open.** Between launch and the first
  transfer, the shell is live. → Harmless in this slice: every control that acts on an image is already
  disabled, and the ones that are not - Browse images, Settings, About - are inert until their own slices. It
  becomes real when the first of those is wired up, and belongs to that slice.
- **A report arriving after the command resolves is drawn onto a dialog that is going away.** The channel's
  handler and the promise's resolution are two paths. → The store keys rows by name and the dialog is
  unmounted on `ready`, so a late report updates state nobody renders. No ordering is relied on.
- **The plan's length is the probe's conclusion, and a machine whose adapters cannot be enumerated is told it
  needs one component.** → Pre-existing and recorded in slice 3 as a parity note for the Settings screen; this
  slice inherits it rather than introducing it, and shows the plan it was given.
- **The overall bar can sit still for a long time on a single-component launch.** macOS installs one archive,
  so the overall bar and the row bar are the same number. → Accepted; the design's own layout has both, and
  the alternative is a dialog that changes shape by platform.
- **StrictMode's double mount is caught in Rust rather than in React.** The effect genuinely runs twice in
  development, and the second `invoke` is a real IPC round trip that returns quickly. → Accepted, and
  deliberately not solved with a module-scope guard on the frontend: the same overlap arrives from slice 5's
  Try again and from a reloaded webview, both of which a React-side flag would not see. The lock is where the
  rule belongs.
- **`prepare_library_path` on Linux replaces the process image before Tauri exists, and this is the first
  binary to do it.** A failure there is reported and startup continues, but a *successful* re-exec of a
  bundled application is exercised nowhere in this project yet. → `opai`'s `tests/reexec_linux.rs` covers the
  mechanism against a test binary; what is untested is a real packaged launch, which is a manual check on the
  Linux leg and is recorded as such below.

**Platforms and execution providers.** Developed and verified on macOS, where the plan is one row - the ONNX
Runtime - and no NVIDIA library is published, so the multi-row dialog, the queued state and the overall
weighting cannot be exercised in a real run there. They are covered instead by frontend tests driving the
store with synthetic four-row plans, which is the same substitution slice 3 made for `Plan::select`. Windows
and Linux are **unverified in a real launch** for this slice: the four-row dialog, the Windows `PATH`
extension and the Linux re-exec-then-initialize sequence are all untested end to end. **No execution provider
is involved**: this change runs no model, builds no session and loads no runtime - it starts the process that
does and reports on the transfers that precede it.
