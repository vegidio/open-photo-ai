## Why

Slice 2 built the window's frame and slice 3 taught `opai` to say what it is about to install and what it
already has - and nothing joins the two. `crates/gui` registers one command, `version`, and never calls
`Opai::initialize` at all. A first launch today opens an empty shell over a machine with no ONNX Runtime on
disk, no GPU libraries, no inference environment and no way for a user to learn that any of that is missing.
`prepare_library_path` has no caller anywhere in the workspace, so even once the libraries arrive the loader
would not look where they are.

Screen 02 is the missing join: the application starts itself, says which components this machine needs, fills
a bar per component as each arrives, and offers the one honest way out of a first launch that cannot finish -
Quit. It is also what every later GUI slice stands on, because a front end that cannot get past
initialization cannot open an image, run a model or export anything.

## What Changes

- **`prepare_library_path` at the top of `main`.** The first statement of `crates/gui/src/main.rs`, ahead of
  the log sink and ahead of `tauri::Builder`, per the safety contract it documents: it mutates the process
  environment and on Linux replaces the process image, both of which are only sound while no other thread
  exists. This is the workspace's first caller.
- **An `initialize` command**, which is the GUI's whole application startup: it calls `Opai::initialize`
  under the identity `opai::GUI`, registers a plan recipient and a progress recipient, and keeps the
  resulting `Opai` handle as managed state for every later slice to reach. It is serialized and its success
  is remembered, so the mount effect, React's StrictMode double-mount and slice 5's Try again cannot start
  two installs of the same archive.
- **Two events to the window**, one carrying the plan and one carrying each progress report, with wire types
  owned by `gui`: `opai::PlannedDependency`, `Progress`, `Dependency` and `Phase` derive no `serde`, and both
  of the latter are `#[non_exhaustive]`, so the boundary needs a shape this crate controls.
- **A `quit` command**, because a first launch that cannot finish has no other exit: the dialog is modal, the
  shell behind it does nothing yet, and the design puts Quit in the dialog's footer.
- **The setup dialog - screen 02.** An overall bar and the count of components installed, one row per planned
  dependency carrying its name, its published size and its state (installed, downloading with its own track,
  or queued), the note explaining that GitHub throttles these downloads, and Quit. It opens on the first
  report that bytes are moving and closes when initialization succeeds, so a launch with nothing to install
  shows no dialog at all.
- **Automatic start.** The frontend initializes on mount without being asked, exactly as the Wails app does.
- **A setup store** holding the plan rows and each row's progress, keyed by dependency.
- **Two generated shadcn primitives**, Dialog and Progress, and the ~15 new i18n keys screen 02 needs, added
  to all thirteen catalogues.

**Not in this slice, and it is the boundary with slice 5:** there is no failure dialog, no per-dependency
failure outcome and no Try again. A failed initialization in this slice leaves the dialog standing with the
rows it had, writes the failure to the log, and leaves Quit as the way out. Slice 5 replaces that standing
dialog with screen 02b.

## Capabilities

### New Capabilities

- `gui-setup`: what the graphical application does between launch and being ready to use - the order of the
  startup sequence, that it starts itself, what the setup dialog shows about each component this machine
  needs, when it appears and when it goes away, and the way out of a launch that has not finished. Slice 5
  extends this capability with the failure dialog rather than adding one of its own.

### Modified Capabilities

None. `gui-shell` describes the frame the window always shows; this slice draws a dialog over it and changes
nothing about the four regions, the title bar or which of the frame's controls are live. `initialization`
already requires everything this slice consumes - slice 3 added the plan report and the already-installed
outcome, and no behaviour of `opai` changes here.

## Impact

**Crates touched: `gui` only.** `src/main.rs`, `src/lib.rs`, one new Rust module for the setup state and its
commands, and the frontend under `crates/gui/frontend/`. `opai` is untouched - slice 3 delivered the whole of
what this consumes - and `cli` is untouched, since it initializes nothing today.

Nothing here belongs in `opai`. What this slice adds is a window's dialog, an IPC boundary and the
composition of the library's per-dependency reports into one bar - and that composition is presentation, which
slice 3 refused to do in the library for reasons that still hold: how several dependencies of wildly different
sizes are weighted into one figure, and how "1 of 3 components installed" is worded and pluralised, are
decisions about a dialog. `cli` could not consume any of it; it renders installs as lines of text, which
`perf` already demonstrates against the same two callbacks.

**rust-sak: nothing applies, and nothing here is infrastructure.** The download, the retries, the resume, the
hashing and the hardened extraction are `fetch`, `crypto` and `fs`, and they are already in use *inside*
`opai` on the path this slice reports over; `sysinfo` supplies the GPU probe behind the plan. This change
transfers nothing, hashes nothing and extracts nothing - it relays what those modules already report across
an IPC boundary and draws it. There is no gap to fill and nothing worth extending `rust-sak` with.

**New dependencies: none.** Rust: `serde` with `derive`, `thiserror` and `tauri` are already in
`crates/gui/Cargo.toml`, and the mutex the serialized command needs is `tauri::async_runtime::Mutex`, a
re-export of tokio's, so no runtime crate is added to a front end. Frontend: Dialog and Progress are shadcn
*generated source*, and both sit on the `radix-ui` package slice 1 already installed - no new npm package.

**Deliberate divergence from the Wails app's behaviour**, all of it following the design project, which the
project's Stack section makes the authority for these screens:

- **The dialog lists every component this machine needs, including the ones already on disk, before any of
  them starts.** The Wails dialog renders `Object.entries(downloads)` and so can only show a dependency that
  has already reported - a row appears mid-download and the list grows as it goes. Slice 3 exists to close
  that gap, and this is the consumer it was built for.
- **An overall bar and a component count.** The Wails dialog has neither; it stacks per-dependency bars and
  nothing answers "how much longer".
- **Quit.** The Wails dialog's only button is Try again, disabled until something fails, so a user whose
  first launch stalls has no way out but to kill the process.
- **The GitHub throttling note.** New, and worth the space: the reference implementation's own downloads are
  GitHub release assets that throttle, and without a sentence saying so a slow first launch reads as a hang.

Behaviour that is **not** a divergence and is kept: the application initializes itself on mount, the dialog is
not dismissible by clicking away from it, a repeat call after success answers from the first call's result
rather than installing anything again, and concurrent calls are serialized - the last of which is the Wails
`initMu`, kept for the same reason its comment gives.

**Parity gaps this slice knowingly leaves open:** the failure dialog and Try again (slice 5); the TensorRT
first-run dialog the Wails app shows after a successful initialization, which is a later screen; and the
providers the initialization decided, which nothing reads until there is a Settings screen to show them in.

**Task estimate: 14**, inside the 15-task limit. This change is slice 4 of a series; `design.md` carries the
full slice list.
