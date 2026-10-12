## Context

See proposal.md - Why. What matters here is what already exists on both sides of the boundary, because this slice
adds almost no logic: it is three facts and the crossing.

`crates/gui/src/setup.rs` holds `Setup<T>`, whose one field is *"the lock, the memo and the handle"* - it already
stores the `Opai` the initialization produced. Its `ready` method's doc records why it answers `Ok(())` rather than
the value: *"Nothing in this slice reads the handle - the dialog is drawn from the progress stream - and handing a
clone back to a command that would drop it is a second route to a value this type is the keeper of."* That reasoning
holds; what changes is that there is now something to read *off* the handle without handing the handle over.

In `opai`, all three facts are already public and already final. `Opai::providers()` returns `SupportedProviders`,
documented as *"a report of what the machine can be asked to run on"*, with the honesty property this slice's spec
inherits: `cuda` and `tensorrt` are true *"only where the library was actually installed, not merely where the
hardware was detected"*. `logging::log_path` resolves `<config dir>/<name>/logs/opai.log` through `rust-sak`'s
non-creating `fs::user_config_dir`, and its doc says *"asking is free: nothing is created, no sink is installed and
no record is written"* - which is exactly what makes it callable from a render. `models::catalogue()` is a
`LazyLock<Vec<FamilyEntry>>` whose module doc states the reason it exists: in the reference implementation *"a
parameter's bounds are literals in a React component - `min={0} max={300}` - with nothing connecting them to the
model code, so a control can offer a value the library refuses."*

The design authority for what slice 7 will draw from these is screens 16 and 16b in the Claude Design project. Their
notes: *"A nav list on the left and scrolling sections on the right. Language and processor changes are drafts until
Save"*, and *"One slider per lossy encoder. The scales are not comparable between formats, so each keeps its own
default mark and remembered value."* Nothing in this slice draws either.

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | done |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs | 1 | done |
| 3 | `add-gui-3-init-plan` | `InitOptions::on_plan` and the already-installed report, so the install list, its per-dependency sizes and each dependency's outcome are observable before and during a first run. `opai` only | - | done |
| 4 | `add-gui-4-setup-progress` | Screen 02 - `prepare_library_path` at the top of `main`, the `initialize` command, progress and plan events, the managed `Opai` handle, the setup dialog, automatic start, Quit | 2, 3 | done |
| 5 | `add-gui-5-setup-error` | Screen 02b - the failure dialog, per-dependency outcome, the failure kind on the wire, Try again and Quit | 4 | done |
| 6 | `add-gui-6-settings-ipc` | The three facts the Settings dialog is drawn from, on the wire: the provider report off `initialize`, the log file's location and revealing it, the model catalogue. No UI | 4 | **CURRENT** |
| 7 | `add-gui-7-settings-dialog` | Screens 16 and 16b - the reusable dialog title bar, the persisted settings store with Cancel/Save, the dialog frame and nav rail, and **all fifteen rows** | 6 | pending |

**Slice 6 is this change.** It is ordered ahead of slice 7 rather than folded into it for the reason the project's
slicing rule gives: the two together are about twenty-five tasks, and the seam that splits them cleanly is the
boundary itself. Cutting the other way - dialog first, facts second - would ship three rows that look live and are
not, which is the failure mode the whole Settings series is being ordered to avoid.

The remaining 17 designed screens are not yet cut into slices; they are cut when they are reached, and the table
carries only what has been committed to.

**Crates.** `opai` gains one derive on one existing public type and nothing else. Everything else is `gui`:
`src/setup.rs`, two new command modules, `Cargo.toml`, `capabilities/default.json`, and the frontend's `ipc/` layer
with its tests. `cli` is untouched.

**rust-sak: nothing used, nothing missing.** Covered in proposal.md - there is no infrastructure in this slice.

**New crates: one**, `tauri-plugin-opener`, and **no npm counterpart** — the frontend never invokes it, because
the reveal goes through a command of this crate's own. Decided below.

**Platforms.** Validated on macOS, which is the development machine. Windows and Linux are untested for the reveal
behaviour specifically, which is the one thing here with three implementations behind it; the other two commands are
platform-independent calls into `opai`. The reveal's threading is `cfg`-split per platform and only the macOS arm has
been run - the Linux arm is written against a filed upstream issue rather than against a launch here, and the Windows
arm against what COM requires. Execution providers: none are exercised by this slice - the provider report is read,
not acted on.

## Goals / Non-Goals

**Goals:**

- Every fact slice 7's dialog needs is answerable before the dialog exists, so no row ships inert.
- The frontend carries no list of model names, labels, bounds or provider names of its own.
- Asking any of the three questions changes nothing on disk and starts nothing.
- A repeat setup answers with the report it already has rather than deciding it twice.

**Non-Goals:**

- **Acting on a chosen provider.** There is no `set_execution_provider` seam here and none in slice 7 either:
  nothing in this application runs inference yet, so there is no session to unload and no registry to tell. The
  choice is persisted and the slice that runs a model reads it. The reference's `SetExecutionProvider` exists
  because it has loaded models to evict; when this project has them, that command arrives with them.
- **Analytics.** The reference sends events through a frontend SDK and nothing here does yet. Slice 7 ships the
  switch because a switch needs no backend; sending anything is its own change.
- **The settings store, the dialog, or any component.** All slice 7.
- **Filtering the catalogue for the settings dialog.** See the decision below.
- **A window for the log, or reading it.** The application points at the file; it does not display it.

## Decisions

### The provider report rides on `initialize`'s answer, and `Setup::ready` projects rather than hands over

`ready` gains a projection: the caller supplies what to read off the stored value, and gets that rather than the
value.

```rust
pub(crate) async fn ready<F, Fut, E, R>(&self, initialize: F, read: impl Fn(&T) -> R) -> Result<R, E>
```

so `initialize` reads `Opai::providers()` and `Setup` keeps handing nothing out. The doc comment's existing
objection - *"handing a clone back to a command that would drop it is a second route to a value this type is the
keeper of"* - is answered rather than overridden: no handle leaves, a `Copy` report does.

**Why the answer and not a second command.** The report is decided *inside* the initialization, and it is the only
moment it is decided. A separate `providers` command would have to answer for a state that has no answer - called
before setup finishes, it either blocks on the same mutex (a second way to wait for setup, which the `Setup` doc
exists to prevent), returns an empty report (indistinguishable from a machine with nothing), or returns an error the
frontend has to tell apart from a real one. Riding on the answer has none of those states: the promise that resolves
is the promise that carries it, and the frontend already awaits exactly that promise in one place.

It also means the **retry path is covered for free**. `Setup::ready` already answers a repeat call from the memo, so
the reload, the StrictMode double-mount and the failure dialog's Try again all get the report with nothing
reinstalled - which is the spec's third scenario, satisfied by the type that was already there.

*Alternatives considered.* **A `providers` command reading the managed `Setup`.** Rejected above. **A global event
emitted at the end of setup.** Rejected for the reason slice 4 chose a channel over global events: a listener
registered in parallel with the invoke can miss an event that fires within milliseconds, and this one fires at the
moment the invoke resolves.

### `SupportedProviders` gains `Serialize` in `opai`, rather than a mirror in `gui`

One derive on the existing type, and `gui` sends it as itself.

This looks like it contradicts slice 5, which deliberately kept the `InitError` classification in `gui` rather than
`opai`. It does not, and the difference is the one slice 5's own design named: that classification is *"decisions
about a dialog"* - which grouping gets a paragraph of advice, which gets a button - and it had one reader. This is
not a decision about anything. It is a report the library already documents as existing *for* a front end to read,
and `opai` already derives `Serialize` on `VariantEntry`, `FamilyEntry` and `ParameterKind` for that exact purpose.
A mirror struct in `gui` would be a second spelling of four booleans that has to be edited whenever the first is.

**`#[non_exhaustive]` and the wire.** `SupportedProviders` is `#[non_exhaustive]`, so a provider added later appears
as a new field in the JSON. The frontend type therefore names the four fields it knows and tolerates the rest, which
is what a structural type does anyway - the opposite of the `Dependency` trap this codebase already refuses, where a
front end had to be edited for every variant the library added. A provider the frontend does not know about is one
it does not offer, which is correct until someone teaches it the name.

*Alternative considered.* **A `gui`-side `ProviderReport` mapped from it.** Rejected: four booleans copied by hand,
and the copy silently stops reporting the fifth provider rather than being a compile error.

### `tauri-plugin-opener`'s library for the reveal, but both calls are commands of our own

Both are commands of this crate, and the plugin is a library rather than a participant. The log path was always
going to be ours - it is `opai::logging::log_path(APP_NAME)` plus an error conversion. The reveal was planned as a
direct frontend call to the plugin and cannot be one, which the implementation found by launching it.

The plugin still does the revealing, and the argument for depending on it is unchanged. `quit` was refused a plugin
because it is `app.exit(0)` - *"one line"*. Revealing a path is not one line: it is `NSWorkspace` on macOS, COM's
`SHOpenFolderAndSelectItems` on Windows and a D-Bus call with an `xdg-open` fallback on Linux. The reference
implementation carries exactly those three branches in its `osservice`, and inheriting them is inheriting
maintenance for something the `tauri-apps/plugins-workspace` train already does. Same train as
`tauri-plugin-single-instance` and `tauri-plugin-os`, pinned the same way, no features.

**What cannot be used is the plugin's own `#[tauri::command]`.** It is `async`, so Tauri runs it on a tokio worker,
and all three of its implementations are doing something that must not happen there:

| Platform | What a runtime worker does to it |
|----------|----------------------------------|
| macOS    | **Aborts the process.** `NSWorkspace::activateFileViewerSelectingURLs` raises an Objective-C exception off the main thread, and Rust cannot catch one: `fatal runtime error: Rust cannot catch foreign exceptions`. |
| Linux    | **Panics, and the promise never settles.** The plugin's blocking zbus call cannot drive a runtime from inside one. |
| Windows  | COM - `CoInitialize` and `SHOpenFolderAndSelectItems` - which wants a main-thread apartment. |

The macOS abort is **Apple's regression, not the plugin's mistake**: `activateFileViewerSelectingURLs`'s own
documentation says *"you can safely call this method from any thread of your app"*, which stopped being true in
macOS 26.4. Apple's DTS has acknowledged it as a bug (FB22340592,
[forums/thread/820622](https://developer.apple.com/forums/thread/820622)), and the workaround they give is the main
thread. Reproduced here deterministically on 26.6.2, three launches out of three, with the crash report putting the
faulting thread at `tokio-rt-worker`. The Linux case is filed against the plugin itself and still open
([plugins-workspace#3552](https://github.com/tauri-apps/plugins-workspace/issues/3552)), whose recommended app-side
workaround is the same as this one: wrap the plugin's synchronous Rust API in a command of your own.

So `reveal_log` picks the thread and the plugin does the reveal. One `cfg`, and each arm is a defect in something
else rather than a second implementation - both call the same function:

- **macOS and Windows: the main thread**, via `AppHandle::run_on_main_thread`, with the reply awaited over a channel
  rather than blocked on, so a busy main thread delays the button instead of parking a runtime worker.
- **Linux: `spawn_blocking`**, and deliberately *not* the main thread. Off-runtime is all #3552 needs, and the main
  thread would cost more than it buys: `org.freedesktop.FileManager1.ShowItems` activates the file manager, so a cold
  one can take seconds to answer, and those seconds would be the window not drawing.

**`reveal_log` takes no path.** It resolves the same file `log_path` answers, in the same process, so nothing the
window says decides which file is opened and the two commands cannot be made to disagree. A general reveal arrives
with its second caller; there is one.

**Permissions: none, and the plugin is not registered.** `tauri_plugin_opener::reveal_item_in_dir` is a free
function touching no application state, so leaving `init()` out costs nothing and takes the three broken commands
off the invoke surface entirely - which is better than registering them and relying on `capabilities/default.json`
to withhold a permission. Commands this application registers itself need no capability entry, as the `quit`
decision already records.

**The spec's failure scenario survives this.** On macOS `activateFileViewerSelectingURLs` itself cannot report a
failure - it returns nothing - but the plugin canonicalizes the path before revealing it, so a log file that is not
there rejects with `LogsError::RevealLog` rather than passing silently.

### The catalogue crosses whole and unchanged

The command returns `opai::catalogue()` serialized as it stands: every family, including detection and colorization,
which no settings row will draw.

Filtering it to "the seven families the Settings dialog lists" would put the answer to *which enhancements are
configurable* in `gui`, which is the model vocabulary the catalogue exists to keep out of a front end. It is also
wrong twice over by slice 7: the add-enhancement menu and the options popovers read the same catalogue with a
different subset in mind, and a filter shaped for the settings dialog would be re-filtered or duplicated for each.
The library's own doc already settled the analogous case - detection is listed *"although the reference's frontend
registry has no `dt` row"*, because *"which model detects faces is model vocabulary"*.

The frontend fetches it once and keeps it, rather than per render. It is static for the life of the process by
construction, so a refetch would serialize the same bytes again.

### Where the commands live

`src/logs.rs` and `src/catalogue.rs`, named for their subject rather than for the screen that asked first. A
`settings.rs` holding both would be wrong by slice 7 alone: the catalogue's second reader is the add-enhancement
menu, and a module named after one caller is the thing this project's layout rules already refuse one tier down.
The provider report stays in `setup.rs`, because it is not a command - it is what an existing one answers.

## Risks / Trade-offs

**Widening `initialize`'s return is a wire change with a live consumer.** → Its one caller is `frontend/ipc/setup.ts`,
updated in this change, and the two halves are already pinned from both sides: `setup.test.ts` fixes the command name
on the TypeScript side and `generate_handler![]` on the Rust side. The failure mode is a type error, not a runtime
surprise.

**The catalogue is the largest payload this application has put on the wire.** → Sixteen-plus variants with their
parameters, serialized once per fetch. Fetched once per session and kept, so the cost is a single call at the moment
the settings surface first needs it; nothing here is in a render path.

**`#[non_exhaustive]` on a serialized struct means the wire can grow without the frontend noticing.** → Accepted, and
it is the safe direction: an unknown provider is one that is not offered. The alternative - an exhaustive frontend
type that breaks on a field it does not know - fails the launch over a provider nobody asked for.

**The reveal is untested on two of three shipping platforms, and now has a `cfg` of our own in it.** → The original
form of this risk assumed *"a single plugin call with no branching of our own"*, and the launch check disproved the
premise: the plugin's command crashes this application on macOS and hangs it on Linux, so there is branching whether
or not we wanted it. What is branched is only the thread, and both arms call the same plugin function, so a mistake
in one is a reveal that does not happen rather than two behaviours that drift. The Linux arm follows the workaround
its own upstream issue recommends. The `Show logs` button remains the lowest-stakes place to find out: a failure
there is reported to the user and costs them a manual trip to a directory.

**Depending on an unregistered plugin is unusual enough to be mistaken for an oversight.** → Written down in three
places that a reader would pass through - the `Cargo.toml` entry, the gap in `lib.rs` where `init()` would go, and
`logs.rs`'s module documentation - because the failure mode is somebody "fixing" it by registering the plugin and
granting `opener:allow-reveal-item-in-dir`, which puts the crashing command back within reach of the webview.

**A slice with no user-visible change.** → Deliberate, and there is precedent: `add-gui-3-init-plan` delivered the
install plan and its sizes with no UI at all, so slice 4 could draw something already true. Verification is the
command tests and the frontend binding tests rather than a screenshot.
