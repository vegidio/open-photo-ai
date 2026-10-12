## Slices

Seven changes, in order. Single-instance came first because the changes after it are designed
against its guarantee, and logging came next because the benchmark ahead of it wants a sink to see
the library through. **This change is perf slice 3, change 7 of the seven, and the last of them.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** (archived `2026-09-14`) |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | **done** (archived `2026-09-14`) |
| 3 | `add-logging-1-file` | The sink: `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, daily rotation, the `---` session divider, the session header, the per-record `app=` field, `APP_NAME`, and the initialization path instrumented | 1 | **done** (archived `2026-09-15`) |
| 4 | `add-logging-2-records` | The instrumentation sweep: the enhancement run, the session registry, the installers, the image cache and the model listing, against the records the Go library emits | 3 | **done** (archived `2026-09-15`) |
| 5 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | **done** (archived `2026-09-15`) |
| 6 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 5 | **done** (archived `2026-09-15`) |
| 7 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 6 | **current** |

With this slice the series is finished. Nothing in it is a foundation for a slice 8: what follows
from here — comparing two sweeps, a baseline file, a regression check in CI — was named as a non-goal
in `add-perf-2-harness` and is its own change whenever somebody wants it.

## Context

See proposal.md — Why. Five things read out of the code rather than assumed shape this design:

- **`add-perf-2-harness` built the seam and left it unused.** `sweep::Listener` is already told
  `started` and `finished` per model, with two implementations: `Silent` for `--json` and `Lines`
  for everything else. The sweep does not know which is in use, and the design that put it there
  said in as many words that this is where the viewport attaches. What the trait does *not* yet
  carry is the stage within one model, which the reference's listener does (`phase(p, current,
  total)`) and which is most of what makes a live line worth looking at.
- **A model install reports through the run's callback, not the startup's.**
  `OperationProgress::installing` returns `None` when the inference callback is `None`, so with
  `add-perf-2-harness`'s `on_progress: None` a several-hundred-megabyte model transfer inside the
  warm-up emits nothing at all. `InitOptions::on_progress`, which `startup.rs` does register and
  writes to stderr, covers the ONNX Runtime and the GPU libraries at startup and stops there. This
  is the single largest silence the harness has, and it is inside the warm-up rather than inside a
  timed section — so closing it is free of every measurement concern below.
- **The progress the library composes is already the right shape.** `InferenceProgress` carries the
  operation, a `Stage` of `Installing(Progress)` / `Running` / `Cached`, an `operation_fraction` and
  a `chain_fraction` that never decreases and lands on exactly `1.0`. The install takes the head of
  each operation's range (`INSTALL_SHARE`), multi-pass upscales are weighted by input pixel area,
  and the throttling is inherited from `rust-sak`. A benchmark's chain is one operation, so
  `chain_fraction` and `operation_fraction` agree — but there is nothing here for this crate to
  compose, smooth or clamp. It reads one number.
- **`Operation` clones without allocating.** Every arm is a plain value type — `Upscale` is a
  `UpscaleVariant` and a `Scale` — so the `InferenceProgress` that `emit` builds per report is a
  stack copy. What registering a callback actually costs inside a timed section is one mutex
  lock/unlock and that copy, per tile. This is the fact D2 rests on.
- **The inference runs on `spawn_blocking`.** `chain::run` hands the tile loop to a blocking thread
  and awaits it, so a `select!` in the sweep's own task is free to tick a renderer while a model
  runs. That is what makes D4's structure possible without a second task and without `'static`.

## Goals / Non-Goals

**Goals:**

- A sweep that is visibly working, in the two places where it currently is not: the minutes a model
  download takes, and the minute a CPU run takes.
- A viewport whose cost is arguable from the code rather than assumed to be zero — see D2, which is
  the load-bearing decision here, and the measurement that has to back it.
- One report, two renderers, and a sweep that still does not know which is running.
  `add-perf-2-harness`'s shape holds; this slice adds one method to a trait and changes nothing
  about the measurement loop's order, its releases or its identities.
- Nothing shown that cannot be justified. The estimate covers what has been measured and stops
  there.

**Non-Goals:**

- **Keyboard interaction.** No pause, no skip, no quit key, no raw mode (D6). Ctrl-C already works
  and works the same whether or not anything is drawing.
- **A full-screen application.** No alternate screen, no panes, no scrollback of its own, no mouse.
  A finished model belongs in the terminal's own scrollback, which is what makes it survive the
  process (D5).
- **Colour as information.** Styling may distinguish a failure from a completion, but nothing is
  said *only* in colour — the plain renderer carries the same words, and so does the report.
- **Live statistics.** The distribution is over the timed runs and is rendered once, in the report.
  A median that moves while you watch it invites reading a number that is not final.
- **Widening `opai`.** Every type this needs is public and named in `tests/public_api.rs` already.
  The rule `add-perf-2-harness` set — a gap the harness feels is reported, not patched here — is
  unchanged.

## Decisions

### D1. `ratatui` 0.30, and the viewport is one line that the terminal scrolls past

`ratatui` 0.30.2 with `default-features = false` and `features = ["crossterm", "scrolling-regions"]`.
Default off drops the calendar widget, the macros and the layout cache, none of which a one-line view
uses; `crossterm` is the backend the ecosystem standardises on and the only one that needs no system
library; `scrolling-regions` is what makes `Terminal::insert_before` push a finished model into the
scrollback using the terminal's own scrolling region rather than clearing and redrawing the viewport,
which is otherwise a visible flicker once per model.

`Viewport::Inline(1)` rather than `Viewport::Fullscreen`: the managed area is one line tall however
many models the sweep covers, and everything above it belongs to the terminal.

**Alternatives.** `iocraft` is a React-shaped TUI crate that is real and growing, and it is the wrong
altitude for one line — it brings a component model and a reconciler to a problem whose entire state
is six fields. `crossterm` alone would do this: the view is one line, and `\r`, a clear-to-end and a
write is the whole of it — which is very nearly what `startup.rs` already does for install progress.
It was rejected on `insert_before`, which is the part that is genuinely fiddly (a finished line has
to be inserted *above* a viewport that is itself being redrawn, without either flickering or
scrolling the other away) and which `ratatui` has already solved behind a feature flag. Writing that
by hand is the kind of code that works on one terminal.

**No spinner crate.** `throbber-widgets-tui` 0.11.1 is maintained and is a `ratatui` widget, and a
spinner is `const FRAMES: [&str; 10]` indexed by a tick counter. This is the same judgement
`add-perf-2-harness` made about `statrs` for five statistics: a dependency whose whole content is a
line you have to write down anyway. `ratatui`'s own `Gauge` is used for the bar, because that one is
not a line — it is Unicode block-fill arithmetic against a cell width, in both a light and a dark
terminal.

### D2. The per-tile bar is fed on every run, and what that costs inside a timed section

`ProcessOptions::on_progress` is registered for **every** run — warm-up, cold start and timed runs
alike — and the callback body is exactly this: read `chain_fraction`, store it into an
`AtomicU64` as bits, and where the stage is `Installing` store the dependency's byte counts into a
second slot. On a run's own arm it writes nothing, locks nothing of this crate's, and allocates
nothing. The renderer polls those atomics on its own timer (D4) and does the drawing.

**The install arm is the one exception, and it cannot be reached from inside a timed section.** D9's
line has to name the dependency, not only count its bytes, so that arm takes a mutex and allocates a
`String` — once per transfer rather than per report, since the name is written only where it changed.
Neither cost is charged to a measurement: a model is put on disk by the **warm-up**, so no timed run
can enter this arm, and where a sweep asked for no warm-up at all the cold start it would be reached
from is exactly the figure the report already warns may cover a transfer. The invariant that holds
everywhere is the one below — nothing *writes to a terminal* from inside a timed section — and the
stores-and-returns property is asserted over the run's own arm alone, which is the scope of the test
that enforces it.

**This refines `add-perf-2-harness`'s D6, which said no callback would be registered on a run, timed or
otherwise.** What that decision was protecting is stated in its own text: *"anything that writes to a
terminal from inside the timed section is inside the measurement"* — and that rule is kept exactly.
What changes is the conclusion drawn from it, because a store is not a write, and
`add-perf-2-harness` had no bar to weigh against it.

**What the measurement is actually charged.** Per tile, inside `OperationProgress::emit`: one mutex
lock and unlock, a handful of `f64` operations, a stack copy of `InferenceProgress` (no allocation —
`Operation`'s arms are plain value types), and this crate's two relaxed atomic stores. Against a tile
of convolutional inference, which is milliseconds at best. The reference reached the same conclusion
and wrote it down in the same terms, and its bar is fed from inside its timed sections today.

**It is measured rather than asserted.** A task of this change runs the same model twice at a high
run count, once with the viewport and once with `--plain`, and checks that the medians sit inside
each other's spread. If they do not, the finding is that this decision was wrong, and the fallback is
to feed the callback only outside the timed sections — which costs the bar during the runs and
nothing else, because the install reporting that closes the largest silence is in the warm-up either
way.

**What the measurement found, and why the fallback was not what fixed it.** The first comparisons
failed the check, and failed it in the direction that says the callback was not the thing being
measured: the *watched* arm was consistently **faster**. Two confounds were removed before anything
was concluded — running the arms A-then-B reads this machine's drift as a result, so they were
interleaved `A B B A`; and running one arm on a pty and the other on a pipe is itself worth 5%,
measured directly with no callback registered in either arm. With both arms on a pty and interleaved,
the gap was **−4.88% on the CPU provider with the medians cleanly separated** — a failure of the
check, but not one the fallback would explain.

So the fallback was built and measured rather than adopted on the strength of the rule. With the
callback deleted from every timed run, the gap was **still there, at −3.36%**. The callback is
therefore not the cause, and D2 stands as written: what a run is charged for reporting itself is not
resolvable against the variation this machine already has.

What remained between the two arms is that one of them woke its task fifteen times a second and the
other sat at an `await` for the length of a run. That is D10.

**Relaxed ordering, and one `AtomicU64` rather than a lock.** There is nothing to synchronise: the
bar is a number that is allowed to be one frame stale, and a `Mutex` would put the inference thread
at risk of waiting on a renderer that is mid-draw. `f64::to_bits` into an `AtomicU64` is the same
device the reference uses and for the same stated reason.

**`Stage::Cached` is shown as itself.** With `--cache` on, an operation served from the store reports
`Cached` at the full share. `add-perf-2-harness` already treats a run that executed nothing as a
finding rather
than a timing (`Verdict::NothingExecuted`), and the viewport saying *"from an earlier run"* is the
same finding made visible a minute earlier.

### D10. Every renderer is paced by the same ticker, including the two that draw nothing

`main` polls the sweep beside a `FRAME`-rate ticker **whatever renderer was chosen**. Under the
viewport the tick draws; under `Lines` and `Silent` it calls a closure that does nothing. The two
that draw nothing still keep the beat.

This exists because of what D2's measurement turned up: with the callback ruled out, a watched sweep
was still about five percent faster than an unwatched one on the CPU provider, reproducibly. The only
remaining difference between them was the wake-up rate of the sweep's own task, which is what a
scheduler reads as a process that does or does not need a performance core. Giving every renderer the
same beat makes the two the same:

| Provider | watched vs `--plain` | medians overlap |
|---|---|---|
| CPU, 2x, before pacing | −4.88% | no |
| CPU, 2x, callback deleted from every timed run | −3.36% | no |
| **CPU, 2x, both renderers paced** | **−0.84%** | **yes** |
| **CoreML, 4x, both renderers paced** | **+2.11%** | **yes** |

It also made the benchmark better at its job, which was not the point but is the stronger argument
for keeping it: across the CPU comparison both arms got faster (≈15.4s → ≈13.4s) and the run-to-run
deviation fell by three to six times (≈364ms → ≈112ms on the plain arm). A measurement whose spread
is a third of what it was is worth more, and the reason is that the process is no longer drifting
between core types while it is being timed.

**The alternative was to explain it away.** The capability requires that a sweep watched and a sweep
unwatched are *"not distinguishable from the other by its numbers beyond ordinary run-to-run
variation"*. A footnote saying they differ by five percent would have been the report apologising for
itself. A timer firing an empty closure costs nothing worth naming and makes the requirement true.

**The mechanism is inferred; the effect is measured.** Nothing here names a platform or a core type,
and nothing depends on the explanation being right: the rule is that what a sweep measures does not
depend on which renderer was watching, and the implementation of that rule is that they all run the
same way. Whether Windows and Linux show the same effect is untested, as the platform note below
already says of everything else in this slice.

### D3. The listener gains a stage, and that is the whole of the sweep's change

`Listener` gains one method beside `started` and `finished`:

```rust
fn stage(&mut self, stage: Stage);   // Installing is the library's own; the rest are the loop's
```

where the harness's own stage enum is `Warmup { run, of }`, `Cold`, `Timed { run, of }` and
`Run { elapsed }` for a finished timed run. The sweep calls it at each of the four points the loop
already has — which is the reference's `observer.phase(...)` at the identical four places — and
nothing else in `sweep.rs` moves: not the release points, not the order, not the per-run identities,
not the `Enhanced` drop, not the classification.

The install stage is **not** reported through this method. It arrives on the progress callback as
`Stage::Installing(Progress)`, from the library, on a different thread, and it carries the
dependency's name and byte counts. Routing it through the listener would mean the sweep learning
about installs, which it has no other reason to know about.

**`Silent` gains an empty body and `Lines` ignores it**, so `--json` and a redirected run behave
exactly as they do today. The per-model lines `Lines` writes are the ones already in the codebase,
unchanged: this slice adds a renderer, it does not rewrite the one that exists.

### D4. `select!` in the sweep's own task, rather than a spawned sweep and a channel

The reference runs the sweep on a goroutine and the Bubble Tea program on the main one, with results
travelling back on a channel — because Bubble Tea owns the main loop and there is no other shape
available. Here there is:

```rust
let sweep = std::pin::pin!(sweep::sweep(..., listener));
loop {
    tokio::select! {
        result = &mut sweep => break result,
        _ = ticker.tick() => view.draw()?,
    }
}
```

The sweep future is polled by the same task that draws, so nothing is spawned, nothing needs
`'static`, and every borrow `add-perf-2-harness`'s signature takes — `&Opai`, `&Input`,
`&[Selected]`, `&Options`, `&CancellationToken` — stays a borrow. The inference is on a
`spawn_blocking` thread, so the sweep future is parked at an `await` whenever there is work happening
and the ticker gets its turn.

The view's mutable state cannot be reached from both arms, since the listener is borrowed by the
sweep future for its whole life. So the listener owns an `Arc` of the shared state and the draw arm
holds another: a `Mutex` for the fields the listener writes between runs, and the atomics of D2 for
the fraction the library's callback writes from a blocking thread. The mutex is never held across a
draw's I/O — the state is copied out under the lock and the frame is composed from the copy — so a
listener call from the sweep can never wait on a terminal write.

**Frame rate: 15 per second.** The reference uses 20. Below about 10 the spinner reads as stuttering;
above about 20 nothing is gained on any terminal and the cost is one full-width line of escape codes
per frame over what may be an SSH connection. Either is defensible; this picks the lower end of the
range that looks right, because the thing being watched takes minutes.

### D5. A finished model goes into the scrollback, not into the view

`Terminal::insert_before(1, ..)` from the `finished` callback, writing the same one-line summary the
`Lines` renderer writes — model, outcome, cold start and median, or the reason it was declined,
failed or interrupted.

This is what the inline viewport is *for*. The alternative — keeping finished models in the view and
growing it — would cap the sweep at a screenful, would redraw every finished row on every frame, and
would take the whole history down with the viewport when the program ends. Inserted lines are the
terminal's, not the program's: they survive the last frame, they survive a Ctrl-C, and they scroll
like any other output.

The consequence to keep in mind is that an inserted line is **final**. A model cannot be amended once
it is in the scrollback, which is why the line carries only what is known at that moment and why the
summary table — which is over the whole sweep, and which is where the throughput figures and the
downgrade warnings live — is still rendered at the end by `report::body`, unchanged.

### D6. No raw mode, no alternate screen, no key handling, and therefore almost no terminal state

The only terminal state this slice takes is the hidden cursor, restored by a guard whose `Drop` shows
it again and by a panic hook installed alongside it.

Raw mode exists to read keys, and nothing here reads keys: Ctrl-C reaches the process as a signal,
where `add-perf-2-harness`'s handler already cancels the token the run honours. Under raw mode that
signal would have to be synthesised from a key event instead, and a torn-down terminal — a panic, a
`kill -9`, a crash inside ONNX Runtime, which is native code running models this binary exists to
stress — would leave the shell without an echo. That is a real failure mode for a benchmark that
runs unattended, and the way to not have it is to not enter raw mode.

The cost is that the terminal echoes `^C` into the viewport line on an interrupt. It is the last
frame before the summary, and the summary immediately overwrites it.

### D7. The renderer is chosen once, from three facts, and the report never knows

```
viewport  ⟺  !options.json && !options.plain && std::io::stderr().is_terminal()
```

- **`--json` forces `Silent`**, as it already does. A machine asked for a document.
- **`--plain` forces `Lines`.** `add-perf-2-harness` listed this flag as a non-goal *"because with
  one renderer
  there is nothing to opt out of"*; there are now two.
- **stderr is where the viewport draws**, because `Lines` and the install progress already go there
  and `add-perf-2-harness` fixed that the report is the *whole* of stdout. `std::io::IsTerminal` is
  `isatty`, not a character-device check, so `perftest 2> /dev/null` correctly gets no viewport.

- **stdout must be a terminal too — and this is a limitation found in implementation, not a
  preference.** This decision originally read *"`perftest kyoto > results.txt` stays watchable"*, on
  the assumption that drawing on stderr was enough to make it so. It is not. `Viewport::Inline` has
  to place itself below output that already exists, which means asking the terminal where the cursor
  is; `ratatui-crossterm`'s `get_cursor_position` delegates to `crossterm::cursor::position()`, which
  on Unix writes the `ESC[6n` query to **`io::stdout()`** — hardcoded, ignoring the backend's writer.
  With stdout redirected the query is written into the report file, nothing answers it, and the
  viewport gives up after crossterm's two-second timeout. Measured on a redirected sweep before this
  was fixed: `\x1b[6n` at byte 530 of `results.txt`, and a two-second stall before the fallback. The
  same call also enables raw mode for the length of the query, which D6 says never happens.

  Three fixes were weighed. Hand-writing the query on stderr and parsing the reply off `/dev/tty`
  (crossterm's *reading* side already falls back there correctly, so only the write is wrong) keeps
  the promise, at the cost of exactly the hand-rolled terminal code D1 chose `ratatui` to avoid, plus
  a raw-mode window this design says should not exist. Swapping to the `termina` or `termwiz` backend
  might route the query through the writer, but is a dependency decision D1 already made against.
  What is done instead is to **test stdout as well**, which turns a mess into a clean choice: no
  stray escape in the captured report, no stall, and no terminal mode entered to ask a question
  nobody can hear.

  **The capability's own requirement is not weakened by this** — it asks that a captured sweep is
  *"still watchable while it runs and the captured file is still parseable"*, and a redirected sweep
  still reports a line per model on stderr while the file holds the report alone. What narrows is
  this decision's own sentence, which promised the *viewport* specifically. Before the fix the second
  half of that requirement was actually violated, by the escape written into the file.
- **`--verbose` does not force the plain form**, which in the reference it must, because its `-v`
  installs a `slog` handler onto stderr mid-sweep. Here the library's records go to the file
  `logging::init` owns and `--verbose` only adds the per-run listing to the report printed after the
  sweep. Nothing writes to stderr during a sweep except the renderer itself.

The header is printed to stdout before the sweep as it is today, and `report::body` and `json::render`
are untouched. **The report is byte-for-byte identical whichever renderer ran**, which is a spec
requirement and is testable without a terminal, because both renderers are inputs to the sweep and
neither is an input to the report.

### D8. The estimate is the current model's remaining runs and refuses to be more

Shown only once at least one timed run has completed, as
`(remaining_runs + (1 - in_flight_fraction)) × median_so_far`, where `median_so_far` is over the
timed runs of *this model* and the in-flight fraction is the atomic of D2. The cold start is excluded
from the median deliberately — it is several times a steady-state run by construction, which is the
whole reason it is measured separately, and including it would overstate every estimate.

It does not span models. What Kyoto at 4x costs says close to nothing about what the next family
costs when the next family has a pipeline — the reference's own numbers spread over an order of
magnitude — and a figure carrying that error would be presented with an authority it does not have.
The model counter already answers "how far through the sweep am I"; this answers "how long until
this row appears", which is the question with an answer.

The reference has no estimate at all, so there is nothing to diverge from. It is named in the series
plan, which is why it is here.

### D9. What the line says, and how it degrades

```
  ⠹ kyoto        (1/1)  run 3/5         ████████████░░░░░░░░  last 412.3ms  ~1.2s
  ⠹ kyoto        (1/1)  downloading     ██████░░░░░░░░░░░░░░  Kyoto 4x — 41 MB of 128 MB
```

Left to right: spinner, codename padded to a fixed width, position in the sweep, stage, bar, and then
whichever tail the stage has — the last completed run and the estimate during the timed runs, the
dependency and its byte counts during an install. Durations go through `report::duration`, the same
fixed-width renderer the summary table uses, so a figure watched and the same figure in the table are
spelled identically.

The line is composed to the terminal's width and truncated from the right, so a narrow terminal loses
the estimate first, then the last-run figure, then the bar — in that order, worst to best. Nothing is
wrapped: a one-line viewport that wraps is a two-line viewport that `insert_before` will corrupt.

**The composition is a pure function** of the view state and a width, returning a `String`, and that
is what the unit tests exercise: a terminal is not needed to check that a stage is named, that a
narrow width drops the right things, or that a declined model's finished line carries the library's
reason.

## Crates, dependencies and validation

**Crate split.** `crates/perf`, and nothing else. `opai`, `gui` and `cli` are untouched, and the
public API is unchanged — `InferenceProgress`, `Stage` and `OnInference` are already public and
already named in `tests/public_api.rs`. New modules under `crates/perf/src/`: the view state and its
line composition, and the viewport that drives them. `sweep.rs` gains the one trait method of D3;
`cli.rs` gains `--plain`; `main.rs` gains the renderer choice of D7 and the loop of D4.

**`rust-sak`.** Nothing, and nothing here is in its territory. Its modules are `crypto`, `fetch`,
`fs`, `image` (with `image-raw`), `memo`, `o11y` and `sysinfo`; none covers terminal rendering, and a
TUI is not a gap worth extending it with — it is presentation for one binary, which is the conclusion
`add-perf-2-harness` reached about its summary table for the same reason. The crate keeps its
existing `fs` dependency for the temp file the embedded sample is materialized through, unchanged.

**Other crates.** One is added: `ratatui` 0.30.2, weighed in D1 against `iocraft`, against `crossterm`
alone, and against `throbber-widgets-tui` for the spinner. TTY detection is `std::io::IsTerminal` and
needs nothing. `comfy-table` keeps its `tty` feature **off** — the terminal question is answered in
D7 by `std`, and a table crate is still not where it should be answered. No change to `clap`,
`serde`, `serde_json`, `thiserror` or `tokio`, whose `macros` feature is what `select!` needs and is
already enabled.

**Ownership and lifetimes.** Nothing here owns a session or touches one. The measurement loop's
ownership model is `add-perf-2-harness`'s D5 and is unchanged by this slice: sequential across
models, every handle dropped before each release, the previous model's `Enhanced` dropped before the
next model starts. What this slice adds is shared *view* state — an `Arc<Mutex<..>>` written by the
listener from the sweep's task and read by the draw arm of the same task, and two `AtomicU64`s
written by the library's callback from a `spawn_blocking` thread and read by the draw arm. The mutex
is never held across terminal I/O, the atomics are relaxed, and neither is on the path of anything
that holds a session.

**Platforms and execution providers.** The line composition, the estimate, the stage labels, the
finished-line rendering, the renderer choice of D7 and the guarantee that the report does not depend
on the renderer are unit-tested beside the code on every CI platform, with no terminal, no runtime,
no GPU, no model file and no network.

What that does not cover is a terminal. The viewport itself, `insert_before`, the cursor restore and
the width degradation are checked by hand on **macOS arm64**, in Terminal.app and over SSH, against
**CoreML and the CPU** — the hardware this project has, the same two providers every slice of this
project has been validated against. The measurement-cost check of D2 is run there too, and is the one
hand-run check whose outcome can change this design. **Windows and Linux terminals are untested**, as
is **Windows Terminal's handling of scrolling regions specifically**, which is the one place
`insert_before` is most likely to behave differently; the fallback if it does is the `--plain`
renderer, which is not a workaround but the documented other half of D7. **CUDA and TensorRT remain
untested here, on any machine**, as they have been in every slice of this project.

## Risks / Trade-offs

**The bar's callback perturbs the measurement after all** → **Measured, and it does not.** Deleting
the callback from every timed run left the watched/unwatched gap exactly where it was, which is the
strongest form this could have been checked in. The gap had a different cause and is closed by D10.
What is left of this risk is the part a measurement on one machine cannot close: the cost is bounded
and readable in the library's own code (one mutex and a non-allocating copy per tile), and the
fallback — feeding the callback only outside the timed sections, at the cost of the bar during the
runs — is still there to reach for if another machine says otherwise.

**The pacing of D10 rests on a mechanism nobody here can observe directly** → What was measured is
the effect, not the cause; core scheduling is inference. Mitigated by nothing depending on the
explanation: every renderer runs the same way, so whatever the mechanism is, it is the same for all
of them. The figures are in D10 so that whoever finds a machine where they do not reproduce can see
what was true here.

**`insert_before` behaves differently on a terminal nobody here has** → Windows Terminal and the
various Linux emulators are untested, and scrolling regions are exactly the kind of thing they differ
on. Mitigated by `--plain` being a first-class renderer rather than a fallback, by the automatic
choice already picking it whenever stderr is not a terminal, and by the exposure being two functions:
the composition is a pure `String` and the only `ratatui` calls are `draw` and `insert_before`.

**A one-line viewport is less than the reference shows** → The reference's line is also one line, so
this is parity rather than a reduction; what is genuinely less is that a narrow terminal drops the
tail rather than wrapping. A wrapped viewport and `insert_before` cannot both be right, and truncation
that loses the estimate before it loses the model's name is the better half of that trade.

**The estimate will be wrong on the first model of a cold machine** → It is derived only from timed
runs, which are after the warm-up and after a release, so the transfer and the cold start are both
outside it by construction. Where it is still wrong — a thermal ramp, another process taking the GPU
— it is wrong in the direction the median already moves, and the report's footer already says the
numbers are comparable within one invocation and not beyond it.

**Two renderers is two things to keep in step** → Mitigated by the report being neither of them: the
table, the JSON and the header are rendered from the sweep's result after it ends, so a renderer can
only affect what was shown *during* the sweep. The spec requirement that both produce the same report
is testable without a terminal precisely because of that split, and the test is written.

**`--verbose` with the viewport is a shape the reference forbids** → Here it is only a longer report
after the sweep, because the library's log is a file. If a later change ever adds a stderr log sink,
this combination becomes the reference's problem and `--verbose` would have to imply `--plain` again.
Recorded so that whoever writes that change finds it.
