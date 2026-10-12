## Slices

Six changes, in order. Single-instance came first because the changes after it are designed against
its guarantee: the logging change puts three binaries in one file and needs to know whether a second
writer is possible, and the benchmark needs to know nothing else is competing for the GPU, the cache
or the model directory while it is measuring. **This change is single-instance slice 2, the last of
that pair.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | **current** |
| 3 | `add-logging-1-file` | `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, the `---` session divider, the session header, the per-record `app=` field | 1 | pending |
| 4 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | pending |
| 5 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 4 | pending |
| 6 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 5 | pending |

Slice 1 was separated from this one by a dependency the project did not have at the time — *"`gui` is
still scaffolding; there is no window to raise."* The scaffold slice supplied that window, so the
dependency is met and nothing else stands between the two. Slices 3–6 do not depend on this one; the
series continues at slice 3 whether or not this lands first.

## Context

See proposal.md — Why for the motivation, and specs/single-instance/spec.md for the behaviour.

Five facts about the existing code and about the plugin shape this design, and were read rather than
assumed:

- **The window exists and is declared, not built.** `tauri.conf.json` carries
  `app.windows[0] = { label: "main", title: "Open Photo AI", width/height 1280x720, minWidth/minHeight
  1280x720, maximized: true }`. `maximized: true` is why D3's refusal to un-maximise matters in the
  common case rather than an edge one.
- **`gui/src/lib.rs` is still the scaffold.** `run()` is `tauri::Builder::default()` with one
  `#[tauri::command]` — `version()`, calling `opai::version()` — and `generate_context!()`. Nothing in
  the crate calls `Opai::initialize`, so nothing in it can be refused by slice 1's claim. D9 says what
  that means for this change and what seam it leaves.
- **`capabilities/default.json` grants `core:default` to `main` and nothing else**, written that way
  deliberately — *"so the first plugin arrives as a visible addition to a minimal list instead of into
  an absent file."* D6 is the answer to whether this is that plugin.
- **The plugin's detection runs inside a Tauri plugin `setup` hook**, which Tauri runs during
  `Builder::build()`, and on every platform the second process leaves from inside that hook via
  `std::process::exit(0)` — preceded by `app.cleanup_before_exit()` on Linux and Windows. This is what
  makes "registered first" a correctness property rather than a style note (D2).
- **The identifier is the key on all three platforms.** `config.identifier` is `io.vinicius.opai`,
  which `crates/gui/src/lib.rs`'s existing drift guard already pins as *"the Go app's bundle
  identity"*. The plugin derives its D-Bus name, its Win32 class/window/mutex names and its macOS
  socket path from it, so that test is already load-bearing for this change and gains a reason to be.

## Goals / Non-Goals

**Goals:**

- A second launch of the GUI ends in the user's existing window, not in a message.
- A second launch is never a failure: successful exit, nothing printed, nothing shown.
- The activation can fail without the application failing. Every degraded path ends in *"started
  normally, no raise"*, never in *"did not start"*.
- What the raise does to the window is exactly what Go's does — including what it refuses to do.
- The seam for the change that wires `Opai::initialize` into the GUI is stated here rather than
  discovered there (D9).

**Non-Goals:**

- Taking the single-instance claim in the GUI. It needs `Opai::initialize`, which downloads a runtime
  and models and needs a startup surface to report progress on; D9 records the ordering constraint it
  must respect and nothing more.
- Using what the second launch supplies. The plugin hands over `argv` and the working directory, which
  is the raw material for open-with-file and deep-link activation. Neither is in this project yet and
  the spec pins the current answer: a second launch changes nothing but which window has focus.
- Any in-process window management — a second window, a tray icon, a re-create-the-window-if-closed
  path. The application is single-window, which is the premise the Go comment argues from.
- Making the raise work where the platform cannot support it. A skipped activation is a supported
  outcome, not a bug to engineer around.

## Decisions

### D1. `tauri-plugin-single-instance`, not a channel built on slice 1's lock

The raise needs something slice 1's mechanism structurally does not have: a way to *tell* the running
process. An advisory file lock answers exactly one question — may I proceed — and the kernel answers
it with a bit. It cannot carry a message, and the refused process has no handle on the holder beyond
the process id in the holder record.

Building that channel locally would mean a Unix socket, a Windows named pipe and their path/name
conventions, a listener wired to the Tauri event loop, and a stale-endpoint story on each platform —
for a feature whose entire payload is *"someone launched you again"*. The plugin is that work already
done, inside the toolkit whose window handles the message has to reach.

It is also the mechanism this project already chose. `add-scaffold-2-gui`'s design D7 names
`tauri-plugin-single-instance` as the Tauri equivalent of Wails' `SingleInstance`, deferred rather than
rejected. This change implements that row; it does not reopen it.

**Version: 2.4**, from `tauri-apps/plugins-workspace` — the same repository and release train as the
Tauri 2 core, released 2.4.4 in August 2026 and on a steady cadence before that. Pinned as `"2"` in
the manifest, matching how the sibling first-party Tauri project pins every plugin it depends on. The
`3.0.0-alpha.0` published in September 2026 is not taken: a pre-release of a plugin whose failure mode
is *at process launch* is not where this project should be early.

**Rejected alternatives:**

- **A local IPC channel keyed off the holder record.** Slice 1 already writes `instance.holder` with
  the holder's `App` and pid, so the refused process knows who to talk to. But a pid is not an address,
  and turning it into one means the three-platform endpoint work above — plus a decision about what
  happens when the endpoint is stale, which is the PID-file class of bug slice 1's D1 was chosen to be
  rid of.
- **Signalling the holder by pid.** `SIGUSR1` to the recorded pid has no Windows counterpart, and a
  recycled pid means signalling an unrelated process.
- **A file the running process watches.** A notification channel built out of polling, with its own
  latency and its own stale-file story, to avoid a plugin that already exists.

### D2. Registered first, and the ordering constraint that follows

`tauri_plugin_single_instance::init(..)` is the first `.plugin(..)` call on the builder. The plugin's
own documentation requires it — *"must be the first one to be registered to work well"* — and the
reason is visible in its source rather than left as advice: on all three platforms the second process
calls `std::process::exit(0)` from inside the plugin's `setup` hook. Anything registered ahead of it
does its setup work in a process that is about to vanish. Cheap when that work is cheap; this project
is on a path where it will not be.

**The constraint this puts on later work**, stated here because that is the point of writing it down:
whatever wires `Opai::initialize` into the GUI must keep it **after** the plugin has had its say. A
second GUI that initializes before the plugin runs would take slice 1's claim, be refused by the first
process's lock, and surface `InitError::AlreadyRunning` — the exact outcome this change exists to
replace. `Builder::build()` is the natural boundary: the plugin runs inside it, so `initialize` called
from the app's own `setup` hook, or from a command the frontend invokes, is on the right side of it.
`initialize` called from `main()` before the builder is on the wrong side.

### D3. The raise is Go's three calls, in Go's order, and Go's one refusal

`show()`, then `unminimize()`, then `set_focus()`, on the window labelled `main`.

The order is not arbitrary and the Go comment gives both halves: *"a hidden window cannot take focus,
and UnMinimise does nothing unless the window really is minimised."* Tauri behaves the same way, and
the upstream report of it — tauri#12936, *"Can't set window focus by the single instance plugin when
the window was hidden"* — is closed as not planned, which makes show-before-focus the caller's job
rather than something a future version fixes.

**`unmaximize` is never called**, and this is the decision worth stating loudest because it is the one
an implementer is most likely to get wrong by reaching for the obvious "restore" verb. The Go comment:
*"Restore() is deliberately not used — it un-maximises a maximised window, so someone who simply
launched the app a second time would find their window shrunk."* The window ships `maximized: true`,
so the naive version would shrink the window of nearly every user who triggers this.

**Absent window is a no-op.** `get_webview_window("main")` returns `Option`, and `None` means the
callback fired before the window existed — possible because the plugin claims its name during
`Builder::build()`. Go guards the identical gap with `if win == nil { return }` for the identical
reason, having had to reach for an `atomic.Pointer` to do it; Tauri's registry makes it an `if let`.
The second process has already exited by then, so doing nothing satisfies the spec.

**Each call's error is logged and ignored rather than propagated.** The callback returns `()`, and
there is no caller to return to — the process that asked is gone. A window that refuses to show is a
lost raise, which the spec already permits.

**The callback body is a free function generic over `Runtime`** rather than a closure inline in
`run()`. That is what lets D8 drive it from a test with Tauri's mock runtime; a closure buried in the
builder chain is unreachable from any test.

### D4. The `semver` feature stays off, deliberately rather than by default

The plugin's `semver` feature appends a version-compatibility string to the identifier, so that 1.x and
2.x of an app are treated as different applications. This project needs the opposite, and the Go app
recorded why on the constant that feeds its own mechanism:

> *"A fixed reverse-DNS string rather than anything derived at runtime: Wails builds a Windows mutex
> name, a macOS lock file and a D-Bus name out of it, so a value that changed between releases would
> let two versions run side by side — which is the situation the lock exists to prevent, since both
> would open the same cache directory."*

That reasoning transfers exactly: the configuration directory is resolved from the application name,
not the version, so two versions share one cache, one model store and one log. Off is the default, so
this decision costs no code — which is why it is written down. A default silently accepted is a
default someone later turns on to fix an unrelated upgrade problem.

### D5. A Linux gate that exists to prevent a panic, not to make headless work

The plugin's Linux backend opens its connection like this:

```rust
zbus::blocking::connection::Builder::session().unwrap().name(dbus_name.as_str()).unwrap()
```

`Builder::session()` resolves an address and does no I/O. `zbus::Address::session()` reads
`DBUS_SESSION_BUS_ADDRESS` and, **when it is unset**, falls back to `unix:path=$XDG_RUNTIME_DIR/bus`
(or `/run/user/<euid>/bus`), which always parses. So:

- **No bus running, variable unset** — the address parses, `.build()` fails to connect, and the plugin
  matches it with `_ => {}`: the application starts unguarded, silently. Already correct, already
  graceful, needs no gate.
- **Variable set but unparseable** — `Address::from_str` fails, and that `.unwrap()` is a **panic
  inside `Builder::build()`**, at launch, before any window. The empty string is the realistic shape:
  containers and stripped-down sessions export `DBUS_SESSION_BUS_ADDRESS=` rather than unsetting it.

So the gate is narrow by construction: it decides whether to call `.plugin(..)` at all, on Linux only,
and it errs toward **skipping**, because skipping costs the raise and registering wrongly costs the
launch.

**The rule**, mirroring the first two steps of Go's `hasSessionBus`:

1. `DBUS_SESSION_BUS_ADDRESS` set to a non-empty value that is not `autolaunch:` and that contains a
   `:` transport separator → register. Go skips `autolaunch:` for the same reason godbus does — *"it is
   a request to spawn a private bus, not the address of one that already exists"* — and a private bus
   no other instance shares would leave the name always free and the plugin guarding nothing.
2. Variable unset → register only if `$XDG_RUNTIME_DIR/bus` or `$XDG_RUNTIME_DIR/dbus-session` exists.
3. Anything else → skip, and log that the application is starting without single-instance activation.

**Two deliberate departures from Go's version**, both recorded rather than silent:

- Go stats `/run/user/<uid>/{bus,dbus-session}`, deriving the directory from `geteuid()`. This reads
  `$XDG_RUNTIME_DIR`, which is what that directory is called on every logind session and what `zbus`
  itself falls back to. The reason is to add no dependency: `geteuid` is not in stable `std`, so
  matching Go literally would mean `libc` (and an `unsafe` block, against a workspace that warns on
  undocumented ones) or `rustix`, for one integer. The case this misses is a session with
  `XDG_RUNTIME_DIR` unset, `DBUS_SESSION_BUS_ADDRESS` unset and a live bus at `/run/user/<uid>/bus`;
  there, the raise is skipped when it could have worked. That is the harmless direction.
- Go's third step — shelling out to `dbus-launch` — is not mirrored, for the reason Go gives for not
  mirroring it either.

**The residual**, stated rather than implied: a `DBUS_SESSION_BUS_ADDRESS` that is non-empty and
carries a `:` but names a transport `zbus` does not know still reaches the panic. Closing that would
mean keeping an allowlist of `zbus`'s transports in this repository, which would age against `zbus`
without anyone noticing. The shapes that actually occur — the empty variable and the absent bus — are
both closed.

### D6. No capability grant, no frontend, and the webview is never told

**No capability grant.** `add-scaffold-2-gui`'s D7 predicted *"a dependency and a capability grant"*.
The dependency is real; the grant is not. The plugin exposes no commands and no JavaScript API — its
whole surface is a Rust callback — so there is nothing for `capabilities/default.json` to permit.
`core:default` on `main` stays exactly as written. Adding an empty grant to match the prediction would
be adding a permission for a thing that cannot be invoked.

**No frontend work at all** — no component, no i18n key, no Tauri command, no event emitted into the
webview. The project's task rule that a user-visible change gets frontend tasks is satisfied by there
being nothing for the frontend to do: the raise is three calls on a window handle, and the DOM inside
that window is unchanged by it. Go emits no event either. This is worth stating because "user-visible"
and "frontend" come apart here, and the rule should be seen to have been applied rather than skipped.

**`argv` and `cwd` are bound as `_args` and `_cwd`.** Naming them out is how the spec requirement that
a second launch changes nothing is held: the values arrive and are dropped. When open-with-file or a
deep link eventually wants them, the change that adds it also amends that requirement, rather than
quietly starting to act on data that was always arriving.

### D7. What the three backends actually are, and where each one's scope differs

Read from the plugin's source rather than from its documentation, because the scoping claims in
specs/single-instance/spec.md rest on it and the three mechanisms reach them differently:

| | Mechanism | Scope it gets | Known gap |
|---|---|---|---|
| **Linux** | A D-Bus well-known name, `io.vinicius.opai.SingleInstance`, on the session bus, with `replace_existing_names(false)`; `NameTaken` means second, and the second calls `ExecuteCallback` on the holder before exiting | Per user, because the session bus is | D5's gate, and Flatpak/Snap needing the D-Bus socket in their manifests if this is ever packaged that way |
| **Windows** | A named mutex `io.vinicius.opai-sim` in the session-local namespace (no `Global\` prefix), plus a hidden message-only window found with `FindWindowW` and sent `WM_COPYDATA` | Per login session, which is per user on a shared machine | If the mutex is taken but the message window not yet created — the holder is mid-startup — the second process launches normally, unguarded |
| **macOS** | A Unix domain socket at `/tmp/io_vinicius_opai_si.sock`; connect succeeds → second, `NotFound`/`ConnectionRefused` → first, anything else → start normally | **Not** per user: `/tmp` is machine-wide | A stale socket owned by another user cannot be unlinked (`/tmp` is sticky), so `bind` fails and this user starts without the raise |

The macOS row is the one that needs saying out loud, because it is the opposite shape to slice 1's
D10, which rejected the temp directory precisely because it *"is neither per-user on every platform nor
guaranteed to survive"*. The per-user requirement still holds in practice, and by accident rather than
by design: the socket another user's process created is not writable by this user under a default
umask, `connect` fails with a permission error, and the plugin's `_ =>` arm starts the application
normally. So two users get two applications and no cross-raise — the right outcome, reached by a path
that is not load-bearing. The cost of the same accident is the stale-socket row: a raise this user
cannot get because another user's file is in the way.

None of this is worth working around here. Every deviation lands on *"started normally, no raise"*,
which the spec permits, and the invariant they would otherwise be guarding is `opai`'s lock's job — as
soon as D9's wiring lands.

### D8. What is tested, and honestly what a test can reach

Three layers, because the three parts of this change are testable to very different depths.

- **The session-bus decision is a pure function and is not `cfg`-gated.** It takes the address as
  `Option<&str>` and the runtime directory as `Option<&Path>` and returns a `bool`; only its *caller*
  is `#[cfg(target_os = "linux")]`. That is deliberate: gating the logic would mean it is only ever
  compiled and checked on the Linux CI leg, and this is the part of the change whose failure mode is a
  crash at launch. A table test covers unset-with-socket, unset-without, empty, `autolaunch:`, a
  separator-less value and a well-formed `unix:path=...`, and it runs on macOS and Windows too.
- **The raise runs against Tauri's mock runtime.** `tauri::test::mock_builder` with the `test` feature
  as a dev-dependency gives a `MockRuntime` app the free function from D3 can be called with, which
  reaches the real `get_webview_window` lookup and the real call sequence. What it proves is that the
  path runs and returns for a window that exists, and that it does nothing and does not panic when
  `main` is absent — the case D3 guards. What it **cannot** prove is that a real window was shown,
  un-minimised or focused: `MockRuntime`'s window methods are no-ops that succeed. Saying so is the
  point; a mock-runtime test that looked like it verified focus would be worse than none.
- **The identifier is already pinned** by `crates/gui/src/lib.rs`'s existing drift guard, which asserts
  `tauri.conf.json`'s `identifier` is `io.vinicius.opai`. All three backends key on it, so that test now
  guards single-instance activation as well as bundle identity, and its comment should say so.

**What no automated test in this repository reaches**: a real second process on each platform raising a
real window. That is a manual check on macOS, and unverified on Windows and Linux — the Platforms
section says so rather than implying the matrix covers it.

### D9. The GUI still does not take the claim, and that is left explicit

After this change the GUI raises a second GUI and is still not subject to slice 1's claim, because it
still never calls `Opai::initialize`. Three consequences, written here so the next change inherits them
as a list rather than as a surprise:

- **A second GUI is handled, by the plugin, without the claim.** The spec's requirements above are all
  about GUI-to-GUI activation and none of them mention the claim.
- **A GUI launched while the CLI or the benchmark holds the claim is not refused**, and
  `InitError::AlreadyRunning` is rendered nowhere. The series' motivating scenario — an overnight
  `perftest` sweep and someone opening the GUI — is therefore still unguarded on the GUI side.
- **The two mechanisms will coexist, and that is the intended end state**, not a duplication to
  collapse later. They answer different questions: the plugin answers *"a second GUI — raise the first
  one"*, the lock answers *"a second process of any kind — refuse it"*. The plugin runs first and takes
  GUI-to-GUI off the lock's hands, so the two never disagree about the same launch. D2 states the
  ordering that keeps it that way.

### D10. Division across the crates

**All of it in `gui`, and this is the residue that belongs there.** The invariant lives in `opai`,
where slice 1 put it. What is added here is window plumbing: find a window, show it, focus it. `cli`
has no window, the `perf` binary ahead of it has no window, and neither would link a Tauri plugin. The
project's test — *"if both `gui` and `cli` would need it, it belongs in `opai`"* — points the other way
for the first time in this series, which is what `gui` exists for.

**`opai` is untouched.** No new type, no new error variant, no change to `initialize` or to
`instance.rs`. `App::Gui` is not passed anywhere new, because nothing in `gui` initializes yet (D9).

**`cli` is untouched**, and stays correct without change: a second CLI is refused by the claim, which
is the right ending for a terminal.

**`rust-sak`: nothing used, nothing owed.** No new feature is enabled and no gap is being worked
around. The `fs` module was read again — temp handles, config directories, copy/move, the three
extractors — and cross-process window activation is not adjacent to any of it. Unlike slice 1's D9,
which recorded the lock as liftable debt because a file lock *is* general-purpose, nothing here could
move: a `rust-sak` that knew about `AppHandle` and `WebviewWindow` would be a Tauri crate.

**Model sessions, memory budgets and execution providers: untouched.** The project's rule that
anything near them describes its ownership and lifetime model applies vacuously — this change runs
before any session exists, holds no handle to one, and the plugin's own state (a D-Bus connection, a
mutex handle, a socket listener) is owned by Tauri's managed state and released by the plugin's own
`RunEvent::Exit` handler.

## Risks / Trade-offs

**A misconfigured `DBUS_SESSION_BUS_ADDRESS` can still panic at launch** → D5 closes the empty and
separator-less shapes, which are the ones that occur; a non-empty value naming an unknown transport
still reaches the plugin's `unwrap()`. Not defended further, because the defence is a transport
allowlist that would age against `zbus` silently. The failure is loud and at launch, which is the best
kind of failure for this to be.

**On macOS the raise depends on a socket in a machine-wide `/tmp`** → D7 has the shape. Two users do
not raise each other's windows, but one user's stale socket can cost another user the raise until it is
removed or the machine reboots. Accepted rather than worked around: the alternative is not using the
plugin on macOS, and the cost of the accident is one extra window.

**The plugin runs before the window exists, so a very fast second launch raises nothing** → D3's
`None` branch. The window is created during the same `Builder::build()` that runs the plugin's setup,
so the gap is milliseconds and needs a second launch inside it. The outcome is a second process that
exits and a first process that carries on starting, which is the spec's own answer.

**On Windows a second launch during the holder's startup is unguarded** → The mutex is taken but
`FindWindowW` finds no message window, so the second process launches normally and two GUIs run.
Nothing here defends it, and the defence already exists in the design: once D9's wiring lands, the
second process reaches `Opai::initialize` and slice 1's lock refuses it. Until then this is the same
window of exposure the project has had all along.

**Two mechanisms for one invariant** → Which slice 1's proposal warned against — *"Fixing them one at
a time means three mechanisms guarding one invariant."* The defence is that these two do not guard the
same thing: the lock is the invariant and the plugin is an activation gesture layered over it, ordered
so that they never answer for the same launch (D2, D9). What would be a real duplication is a second
*refusal* mechanism, and this change adds none.

**A new dependency at the most fragile point in the process's life** → The plugin's code runs inside
`Builder::build()`, before anything is on screen, so a bug in it is a bug at launch. Mitigated by
taking the stable 2.4 line rather than the 3.0 alpha, by pinning it as `"2"` alongside the Tauri core
it ships with, and by D5 gating the one failure path in it that is known to abort rather than degrade.

**The GUI is still not subject to the claim** → D9, and deliberate. The alternative was folding
`Opai::initialize` into this change, which drags in a startup progress surface for a ~175 MB runtime
download and an error dialog, and takes the slice past the fifteen-task limit.

## Platforms

The design targets **macOS, Windows and Linux**, and each gets a different plugin backend (D7) rather
than one mechanism three times — which is why D7 exists at all and why the per-user claim in the spec
is argued per platform.

**Validated on none of them yet** — this is a design. The honest position is the one slice 1 stated:
the only platform this project's live checks have ever run on is macOS arm64. What the tests in D8
reach on every CI leg is the session-bus decision table and the mock-runtime raise; what they do not
reach on any leg is a real second process raising a real window. That is a manual check on macOS and
unverified on Windows and Linux, and the risks above rather than this section are where the
consequences are recorded.

The Linux leg is the only one where a platform-specific *code path* is added rather than selected by
the plugin, and D8 deliberately keeps its logic compiled and tested on all three.

**Execution providers are untouched.** Nothing here resolves a provider, builds a session or reads a
memory budget; the change runs before any of that exists in the process.
