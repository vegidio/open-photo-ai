## Why

The Settings dialog the design draws as screens 16 and 16b has fifteen rows, and three of them cannot be drawn
honestly today. **AI Processor** is a select whose options are the execution providers this machine turned out to
support - a fact `Opai::providers()` already decides during initialization and currently writes only to the log.
**Logs** is a button that opens the log file's location - a path `opai::logging::log_path` already resolves without
side effects, reachable from no front end. The seven **enhancement default-model** rows are selects over the model
vocabulary `opai::catalogue()` already publishes as data, likewise reachable from no front end.

A select with no options is not a control. Rather than ship the dialog with three rows that look live and are not,
this slice puts the three facts on the wire first, so the slice that draws the dialog draws every row working. It is
the same ordering `add-gui-3-init-plan` used before the setup dialog: the observability came first, and slice 4 drew
what was already true.

## What Changes

- **`initialize` answers with what this machine can be asked to run on.** The command returns `SupportedProviders`
  instead of `()`. The value is decided inside the call and stored by `Setup<Opai>` already; what changes is that the
  stored handle is projected through rather than discarded, so a repeat call - a reload, StrictMode's second mount,
  the failure dialog's Try again - answers with the same report without re-installing anything.
- **`SupportedProviders` gains `Serialize`** in `opai`, so it crosses the boundary as itself rather than as a copy
  `gui` maintains beside it. It is already documented as the report *"a front end shows the list from"*, and the
  catalogue's own entries already derive `Serialize` for exactly this reason.
- **A `log_path` command** answering where this application's log file is. It creates nothing and installs nothing,
  which is what lets a settings row ask at render time.
- **Revealing this application's log file in the platform's file manager**, through `tauri-plugin-opener`'s Rust
  API. Unlike `quit`, which this crate refused a plugin for because it is one line, this is three platform-specific
  implementations. The plugin's own command is not used and the plugin is not registered: it runs the reveal on a
  tokio worker, which aborts the process on macOS and hangs it on Linux. See design.md.
- **A `catalogue` command** publishing `opai::catalogue()` unchanged, so no front end restates which models exist,
  what they are called or what they take.
- Frontend IPC bindings and their name-pinning tests for each of the above, and the setup store recording the
  provider report the way it already records the plan.

No user-visible change: nothing in this slice draws anything. What it delivers is that slice 7 can.

## Capabilities

### New Capabilities

- `gui-settings`: what the graphical application's settings surface can know and do - which execution providers this
  machine offers, where its log file is and how a user is shown it, and which models each enhancement can be set to
  default to. The slice that follows adds this capability's dialog requirements; the facts come first because the
  dialog is drawn from them.

### Modified Capabilities

None. `gui-setup`'s requirements cover startup's ordering, its once-per-session rule and the setup dialog; none of
them says what the `initialize` command answers with, so widening that answer changes no requirement there.

## Impact

**Crates: `opai` and `gui`.** `opai` gains one derive on an existing public type and nothing else - no new logic, no
new module. Everything else is `gui`: `src/setup.rs` (the projection and the widened return), one or two new command
modules, `Cargo.toml`, `capabilities/default.json`, and the frontend's `ipc/` layer with its tests.

Nothing is put in a front end that belongs in `opai`. The three facts are all produced by the library and published
by it already; `gui` adds only the crossing. A `cli` wanting the same three calls `opai` directly, which is why none
of this is shared logic in the wrong place.

**rust-sak: nothing is used and nothing is missing.** There is no infrastructure in this slice - no transfer, no
hash, no extraction, no probe, no cache. The one thing that might look like it, resolving the log path, is already
built on `rust-sak`'s non-creating `fs::user_config_dir` inside `opai`, and this slice only calls it. Revealing a
path in a file manager is a desktop-shell concern rather than a general-purpose utility, so it is not a gap worth
extending `rust-sak` with; it is what `tauri-plugin-opener` exists for.

**New crates: one.** `tauri-plugin-opener` from the same `tauri-apps/plugins-workspace` train as the two plugins
already here. No npm counterpart: the frontend never calls it, because the reveal goes through a command of this
crate's own. The alternative - writing the three platform branches in this crate - is what the reference
implementation's `osservice` does and is not worth inheriting.

**Task estimate: fourteen.** Within the fifteen-task limit. The Settings work as a whole is roughly twenty-five, which
is why it is two slices; this is the first.

**Parity.** No deliberate divergence in behaviour. One structural note that is not a divergence to justify: the
reference implementation reaches its provider report through a service call separate from initialization, while this
returns it from the call that decides it. The user-visible result is the same list in the same select.
