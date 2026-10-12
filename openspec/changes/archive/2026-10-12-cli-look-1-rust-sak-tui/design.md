## Context

See proposal.md - Why. Four facts, checked in the code rather than assumed, shape this design:

- **MediaSim's bar is already a small set of separable pieces** (`vegidio/mediasim`, `crates/cli/src/progress/`):
  `bar.rs` (`GradientBar`), `spring.rs` (`Spring`), `eta.rs` (the ETA text), and `mod.rs`. In `mod.rs` the
  `ProgressLine` widget is general, but the `show` loop is tied to MediaSim: it is fed by a `mpsc::Sender<usize>` of
  completed counts, writes only to stdout through `ratatui::try_init`, and ends itself with MediaSim's own
  `CliError::Interrupted`.
- **`opai` keeps stdout empty** (`crates/cli/src/main.rs`; `cli-enhance` spec). Everything it says, progress
  included, goes to stderr. A session that can only use stdout is unusable here.
- **`opai`'s progress arrives from async code and blocking threads.** `OnInference`/`OnProgress` are
  `Arc<dyn Fn + Send + Sync>` called from tokio tasks and from `spawn_blocking` tile loops. Its position is a
  fraction, not a count.
- **`rust-sak` has no terminal module** and no `ratatui` or `crossterm` dependency. MediaSim (`ratatui = "0.30"`) and
  this repository's `perf` (`ratatui 0.30.2`, `crossterm` backend, default features off) are on the same version.

## Slices

The CLI's new look is delivered as three changes. **This change is slice 1.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-look-1-rust-sak-tui` | `rust-sak`'s `tui` feature: `GradientBar`, `Spring`, `ProgressLine`, ETA text and the live one-line session that writes to stdout or stderr; a `rust-sak` release. Here: `opai`'s pin moved to the new tag, and `cli` depending on `rust-sak` with `tui` | - | **CURRENT** |
| 2 | `cli-look-2-progress-core` | In `opai`: the detection-then-chain `Handover` moved from `gui` into the library so both front ends draw one bar the same way, and the operation's index carried on `InferenceProgress` so a front end can count `[n/N]` without inferring it. The GUI's behaviour does not change | - | PENDING |
| 3 | `cli-look-3-terminal` | In `cli`: the new layout (the `💾` dependency line frozen at the last dependency's 100%, the `🖼️` header, the labelled gradient bar using the GUI's labels, and the `✨`/`🧨` result line), `indicatif` removed, notes, face report and provider details taken off the terminal, plain output when stderr is not a terminal, and the `cli-enhance` spec delta | 1, 2 | PENDING |

Slices 1 and 2 are independent and may land in either order. Moving MediaSim to `rust_sak::tui` is outside this
series and happens in MediaSim's own repository.

## Goals / Non-Goals

**Goals:**

- MediaSim's look, pixel for pixel, reachable from any CLI that enables one feature.
- A session that serves both a count-driven producer on a thread (MediaSim) and a fraction-driven producer from
  async callbacks (`opai`), on the stream the caller chooses.
- The terminal is always restored, and the final frame is always left in the scrollback.
- Every widget testable on ratatui's `TestBackend`, so CI checks the drawing on all three platforms without a real
  terminal.

**Non-Goals:**

- Deciding whether to draw. Checking `is_terminal` and `NO_COLOR` is up to the caller. The widgets take
  `color: bool`, and starting a session on a non-terminal is an error, not a fallback.
- Colour palettes and text helpers (`paint`, `bold`, the named RGB constants). These stay in each CLI.
- Viewports taller than one line, `insert_before` scrollback writes, spinners. `perf` has those needs and its own
  viewport.
- An ETA estimator. MediaSim's `Eta` lives in its library crate and stays there; `tui` only formats an
  `Option<Duration>`.

## How the work divides

**`rust-sak` first, then two manifest lines here.** The drawing goes into `rust-sak` because it isn't specific to
Open Photo AI, which passes the project's own test for `rust-sak` code: another project (MediaSim) already wants it.
The `cli` crate is where it gets used, because rendering a terminal is front-end work. Nothing goes into `opai`,
which stays free of anything UI-specific. `gui` is not involved.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Gradient bar, animation, progress line, ETA text | **nothing - this slice adds them** | **`tui` (new)** |
| Live one-line inline session on stdout/stderr | **nothing - this slice adds it** | **`tui` (new)** |
| Errors | `std::io::Error` (everything that can fail here is terminal I/O) | - |

No `thiserror` enum is added: every failure is a crossterm or terminal I/O error, and an error type that only wraps
`io::Error` adds nothing.

## Decisions

### Module layout: one file per public item, like the other modules

`src/tui/` holds `gradient_bar.rs`, `spring.rs`, `progress_line.rs`, `eta.rs`, `live_line.rs`, `animated_progress.rs`,
`live.rs`, `stream_backend.rs` (private; see The cursor position is asked for on the target stream), `mod.rs`,
`tests.rs` and `README.md`. This follows `fs`, `github` and `image`, so the module reads like the rest of the crate.

### `ProgressLine` generalises MediaSim's widget instead of copying it

| Field | MediaSim | `rust-sak` |
|---|---|---|
| label | `&str`, followed by exactly 3 spaces | `&str`, padded to `label_width` (default: its own width), then 3 spaces |
| counter | always `[n/N]`, `n` zero-padded to N's width | `Option<(usize, usize)>`, same formatting when `Some` |
| percent | derived from the counter | given by the caller, floored to tenths, so `100.0%` appears only at exactly 1 |
| ETA | always drawn, `ETA --` when unknown | `Option<Option<Duration>>`: outer `None` leaves the column out, `Some(None)` draws `ETA --` |
| bar width | fits the terminal, clamped to 10..=50 | same |

`label_width` is what keeps the bar from shifting when a caller's label changes (for example from `Upscale` to
`Face Recovery`). MediaSim's labels never change mid-display, which is why it never needed this. The ETA uses
`Option<Option<_>>` rather than an extra flag, so "no column" and "no estimate yet" can't be confused.

**Alternative rejected: keep MediaSim's struct exactly and add a second widget for `opai`.** That gives two widgets
for one line, and they would drift apart.

### `Spring` and `GradientBar` move unchanged

The constants (frequency 18, damping 1.0, a 1/60 s step, settle epsilons) and the gradient endpoints (`#5a56e0` to
`#ee6ff8`, with empty cells `#606060`) are copied as they are, so MediaSim looks exactly the same after it moves over.
They are public so a caller can build a different line from them.

### The live session is generic over what it draws

```
  producer threads / async callbacks          frame thread (owned by Live)
  +-------------------------------+           +------------------------------------+
  | handle.update(|line| ...)     |--Mutex--> | every 1/60 s:                      |
  |   (Send + Sync + Clone)       |           |   poll keys -> Ctrl-C -> on_interrupt
  +-------------------------------+           |   lock, line.render(area, buf)     |
                                              |   draw into Inline(1) viewport     |
  live.finish() ----------------------------> |   until line.settled() or the cap  |
                                              |   restore raw mode, cursor below   |
                                              +------------------------------------+
```

- `trait LiveLine: Send + 'static { fn render(&mut self, area: Rect, buf: &mut Buffer); fn settled(&self) -> bool { true } }`.
  `render` takes `&mut self` so an animated line can step its spring once per frame.
- `Live::start(target: Target, line: L, on_interrupt: impl Fn() + Send + 'static) -> io::Result<Live<L>>`, where
  `Target` is `Stdout` or `Stderr`.
- `Live::handle()` returns a `LiveHandle<L>`. `update(|&mut L|)` locks the shared line and changes it. Shared state
  behind a mutex is used rather than MediaSim's channel because a channel of counts suits only one kind of producer.
  The lock is held only for a field write, or for one frame's render, 60 times a second, so contention is negligible.
- `finish(self) -> io::Result<()>` keeps drawing until `settled()` is true or the settle cap is reached (1 s by
  default, as in MediaSim, and configurable). It then draws a final frame, restores the terminal, puts the cursor on
  the line below and shows it.
- `AnimatedProgress` is the `LiveLine` both CLIs actually use. It holds the label, label width, counter, target
  fraction, percent, ETA and colour flag, plus a `Spring`. Its `render` steps the spring and draws a `ProgressLine`,
  and its `settled()` is the spring's. A plain text line, such as `opai`'s dependency line, implements `LiveLine`
  with a `ratatui::text::Line`, and `rust-sak` provides that as `impl LiveLine for Line<'static>`.

**Alternative rejected: MediaSim's `show(work)`, which runs the work and returns its result.** That inverts control,
so the session would own the work. `opai`'s work is an `async` call owned by the tokio runtime, and putting it inside
a sync frame loop would mean a nested runtime or a channel bridge in every caller.

### The terminal is built by hand, not with `ratatui::try_init`

`try_init` writes to stdout and installs a process-wide panic hook. Instead, `Live` builds
`Terminal::with_options(CrosstermBackend::new(stream), TerminalOptions { viewport: Viewport::Inline(1) })` over the
chosen stream, and enables raw mode itself. A library should not replace an application's panic hook, so restoring
the terminal is done by a guard's `Drop`. It runs on `finish`, on an early return, and on unwinding through the
frame thread's join. A panic under `panic = "abort"` cannot be handled by anyone, and is documented as such.

### The cursor position is asked for on the target stream

Found during implementation. An inline viewport needs the cursor's row, which ratatui gets from
`Backend::get_cursor_position`. On Unix, crossterm answers that by writing `ESC [ 6 n` to **stdout**, whatever stream
the backend draws on. With `Target::Stderr` and stdout redirected, the query never reaches the terminal, so
`Live::start` would time out after 2 s. If stdout were a file, the escape bytes would also be written into it.

`Live` therefore draws through a private `StreamBackend`: a `CrosstermBackend` over the chosen stream that delegates
everything except `get_cursor_position`. On Unix that writes the query to the chosen stream and reads the reply from
`/dev/tty`, waiting with `libc::poll` and the same 2 s timeout. On Windows crossterm reads the console's cursor through
the console API and is used as it is. `libc` becomes a unix-only optional dependency of `tui`. It is already in the
tree through crossterm, so no crate is added.

The guard asks for the cursor once more before restoring, and continues below that row when it is past the viewport.
When `Live` is dropped during a panic, the frame thread skips its last frame, since that frame would move the cursor
back onto the line. Together, these put the shell's prompt below the panic message instead of on top of it. The
message itself is still printed in raw mode, since the panic hook runs before unwinding and this module does not
replace it.

**Alternative rejected: drawing without ratatui's `Terminal`** (render into a `Buffer`, write the row after `\r`).
That needs no cursor query, but it departs from the inline-viewport design and re-implements `CrosstermBackend`'s
cell and style writing.

### Ctrl-C is a key, reported and not acted on

In raw mode Ctrl-C arrives as a key event on every platform, not as SIGINT. The frame thread recognises it and calls
`on_interrupt` once per press, then keeps drawing. Whether to cancel, finish or exit is the caller's decision.
MediaSim cancels its scan and returns `Interrupted`. `opai` will cancel its `CancellationToken` in slice 3.

**Alternative rejected: ending the session on Ctrl-C.** The caller usually wants the bar to stay up while the work
winds down, and a session that removed itself would leave the screen blank during that time.

### Version and pins

`rust-sak` goes to `26.10.6`, following the CalVer the crate already uses. `opai` and `cli` pin that tag, and so do
`gui` and `perf`, which also name `rust-sak` directly, so the lock file holds one `rust-sak`. Two tags would compile
the crate twice and could let the two copies drift apart. For `gui` it would also break the build: `gui`'s manifest
records that two copies make `image::DynamicImage` two unrelated types.

`26.10.5` was released first and does not build in this workspace. `perf` enables ratatui's `scrolling-regions`
feature, features unify, and that adds two required methods to the `Backend` trait that `tui`'s `StreamBackend`
implements. `26.10.6` turns `scrolling-regions` on in `rust-sak` itself, through `ratatui-crossterm` so the lock file
gains no extra backends, and delegates both methods. That leaves the trait the same in every build. Nothing pins
`26.10.5`.

## Risks / Trade-offs

- **[Anything else writing to the stream during raw mode lands on top of the bar, and newlines do not return the
  carriage.]** → Documented on `Live`: while a session is up, the caller must not write to its stream. `opai`'s
  `tracing` output goes to `opai.log`, not to the terminal. Native ONNX Runtime diagnostics are routed through
  `tracing` (`add-logging-1-file`), so nothing is known to write to stderr directly. Slice 3 keeps stdout empty, and
  draws on stderr only while a session is up.
- **[Windows raw mode and inline viewports have only been tested manually on macOS.]** → CI compiles `tui` on its own
  on macOS, Windows and Linux, and the widget tests run on `TestBackend` on all three. The live session itself (raw
  mode, a real terminal, Ctrl-C as a key) is checked by hand on macOS in this slice. Windows Terminal and a Linux
  terminal remain **untested** until slice 3's manual checks.
- **[A narrow terminal cuts off the line.]** → Same as MediaSim: the bar shrinks to 10 cells, then the right end is
  clipped. No wrapping, because a wrapped line would break the one-line viewport.
- **[Emoji width varies between terminals (`🖼️` carries a variation selector).]** → Not this slice's concern:
  `ProgressLine` draws no emoji. Slice 3 prints its emoji lines outside the viewport, as plain text.

## Validation

- Platforms: widget tests on `TestBackend` on macOS, Windows and Linux in `rust-sak` CI. The live session is checked
  by hand on macOS only. Windows and Linux terminals are untested in this slice.
- Execution providers: not applicable. Nothing here touches inference.

## Migration Plan

Additive throughout. `rust-sak` gains a feature nobody has enabled yet. Here, both pins move to a tag whose other
modules are byte-for-byte what `26.10.4` shipped. Rollback means pointing both pins back at `26.10.4`.
