## Why

Slice 1 gave every binary the same ending for a second launch: an error. That is the right ending
for a terminal, where the user typed something and can read why it did not happen. It is the wrong
ending for an icon. Someone who double-clicks Open Photo AI while it is already running has not made
a mistake and is not asking a question — they are asking for their window.

The reference implementation says so at `cmd/gui/main.go:157`:

> *"A second copy of the app is where the image cache's directory lock is genuinely contended: one
> directory, one lock, two processes. The library degrades to an in-memory cache rather than failing
> when it loses that race, but the better outcome is not to race at all — the app is single-window,
> so the second copy was never useful. **Refusing it and raising the window that already exists is
> both the fix and what someone launching the app again actually meant.**"*

And it makes the consequence explicit in the same block: *"ExitCode is 0 because a second launch is
not a failure from the user's point of view; they double-clicked the icon."*

Slice 1 deferred this for one reason and said so — *"there is no window to raise until the GUI has
one"*. The scaffold slice gave the GUI its window: `tauri.conf.json` declares `main` at 1280x720,
maximised on start. The dependency is met, so the deferral is over.

The same scaffold slice is where this work was already assigned. Its design.md, D7, lists the four
Wails window behaviours it deliberately deferred, and one row reads: `SingleInstance` →
`tauri-plugin-single-instance` → *"A dependency and a capability grant whose whole purpose is to focus
a window the user cares about."* That table exists, in its own words, as *"the mitigation for the risk
that a deferral becomes a regression nobody remembers choosing."* This change is the row being paid.

## What Changes

Task estimate: **10**. This is slice 2 of the two-slice single-instance series, and the second of the
six changes design.md's series table records.

**In `gui`, one plugin, one callback and one gate:**

- **`tauri-plugin-single-instance`**, registered as the **first** plugin on the builder — the plugin's
  own requirement, and it is load-bearing rather than stylistic: its whole detection runs in a setup
  hook during `Builder::build()`, and a second process either claims the platform's name or hands its
  arguments to the holder and calls `std::process::exit(0)` from inside that hook. Anything registered
  ahead of it would have run in a process about to exit.
- **The raise, in Go's order and for Go's reasons**: `show()`, then `unminimize()`, then
  `set_focus()` on the `main` window. A hidden window cannot take focus; `unminimize` does nothing
  unless the window really is minimised. `unmaximize` is deliberately never called — the Go comment
  is blunt about it: *"Restore() is deliberately not used — it un-maximises a maximised window, so
  someone who simply launched the app a second time would find their window shrunk."* The window is
  configured `maximized: true`, so this is the common case, not the edge.
- **A no-op when there is no window yet.** The plugin's name is claimed before any window exists, so
  the callback can fire in the gap. Go guards the same gap with `if win == nil { return }`; here
  `get_webview_window("main")` returns `None` and the callback does nothing. The first process is
  unharmed and the second still exits.
- **A Linux session-bus gate**, carried over from the Go app's `hasSessionBus` and kept for a
  narrower reason. The plugin's Linux backend opens `zbus::blocking::connection::Builder::session()`
  and `.unwrap()`s it. `zbus::Address::session()` fails only when `DBUS_SESSION_BUS_ADDRESS` is *set
  but unparseable* — an empty string qualifies, which is a realistic container shape — so that unwrap
  is a panic inside `Builder::build()`. When the variable is unset the address always parses
  (`$XDG_RUNTIME_DIR/bus`) and a bus that is merely unreachable is handled gracefully by the plugin's
  own `_ => {}` arm. The gate exists to keep a malformed environment from crashing the application at
  launch; it does not exist to make a headless box work, because that already works.
- **No new capability grant.** The plugin exposes no JavaScript API, so `capabilities/default.json`
  is untouched. D7's *"a dependency and a capability grant"* was half right and this change says so
  rather than adding an empty grant to match a year-old prediction.
- **No frontend change.** No component, no i18n key, no Tauri command. The raise happens entirely in
  the Rust process and the webview is never told it happened — which is also what Go does.

## Capabilities

### Modified Capabilities

- `single-instance`: adds what a *second launch of the graphical application* does, which the
  capability's own Purpose already covers — *"what the second process is told"* — but which slice 1
  could only answer one way. Today every binary's answer is a refusal; this adds the graphical
  application's answer, which is to exit successfully having raised the window the user was asking
  for. Nothing already written changes: the claim, its scope, its release on death and the refusal
  that names its holder are all untouched, because the GUI does not yet take the claim.

## Impact

**Crates:** `gui` only. `opai` is untouched — no new type, no new error, no change to `initialize`.

This is logic in a front end, and the justification is that there is nothing shared about it: it
raises a window, and `gui` is the only crate that has one. `cli` and the `perf` binary ahead of it
have nothing to raise and would not link a Tauri plugin. By this project's own test — *"if both `gui`
and `cli` would need it, it belongs in `opai`"* — this is exactly the residue that belongs in `gui`.
The invariant itself already lives in `opai`, where slice 1 put it.

**`rust-sak`:** nothing, and nothing that could be. The `fs` module was read again for this change
and covers temp handles, config directories, copies, moves and the three extractors; `o11y` exports
log records; `fetch`, `crypto`, `memo` and `sysinfo` are further away still. Cross-process window
activation is a Tauri-runtime concern — a D-Bus well-known name, a Win32 message window, a Unix
socket, each wired to an `AppHandle` — and a general-purpose crate that knows nothing about Tauri
could not host it. This is not a gap in `rust-sak`; it is outside it.

**Dependencies:** one, `tauri-plugin-single-instance` **2.4**, from `tauri-apps/plugins-workspace` —
the same repository and release train as the rest of Tauri 2, which is what the sibling first-party
Tauri project pins every plugin it uses to. Current stable is 2.4.4 (August 2026); a 3.0.0-alpha.0
exists and is not taken. Its `semver` feature is **left off**, and that is a behavioural decision
rather than a default accepted by inertia: the feature appends the app version to the identifier,
which is precisely what the Go app's comment on `singleInstanceUniqueID` forbids — *"a value that
changed between releases would let two versions run side by side — which is the situation the lock
exists to prevent, since both would open the same cache directory."* No other crate is added; the
Linux gate reads two environment variables and stats two paths, which needs nothing.

**Behavioural divergence from the Go implementation**, all of it forced by what the two toolkits
provide rather than chosen, and each one a narrowing of where the raise works rather than a change to
what it does:

- **macOS scoping.** Wails takes a lock file; the Tauri plugin listens on `/tmp/<identifier>_si.sock`,
  which is not per user. Two people signed into one Mac therefore do not raise each other's windows —
  which is correct, and matches slice 1's per-user claim — but one of them can be left without the
  raise if the other leaves a stale socket behind, since `/tmp` is sticky and the file cannot be
  removed. The failure is a launch with no raise, never a refusal.
- **Windows startup race.** The plugin claims a named mutex, then locates the holder's message window
  with `FindWindowW`. A second launch during the holder's own startup finds the mutex taken and the
  window absent, and launches normally. Go's Wails backend has the same shape.
- **Linux gate, same decision, different reason.** Go disables single-instance entirely without a
  session bus because Wails' `application.New` calls `os.Exit(1)` from inside the constructor,
  skipping every deferred flush — *"turning 'starts fine' into 'exits silently at launch' on a
  headless or containerised session would be a worse bug than the one being fixed here."* Here the
  headless case is already graceful and only a malformed address crashes, so the gate is kept for the
  malformed case and its Go rationale no longer applies.

**Parity gap this change knowingly leaves open**, so that it is picked up rather than discovered: the
GUI still does not call `Opai::initialize`, so it still does not take the single-instance claim. A
second *GUI* is handled here, by the plugin, without the claim. A GUI launched while the CLI or the
benchmark holds the claim is not yet refused, and `InitError::AlreadyRunning` is not yet rendered
anywhere. Both arrive with the change that wires the GUI to `opai` — which slice 1's proposal already
assigned, *"added by the changes that write those binaries"* — and neither is blocked by anything
here. Ordering is the one constraint this change imposes on that work, and design.md records it: the
plugin must keep running before `initialize`, or a second GUI would be refused by the lock before the
plugin could raise the window.
