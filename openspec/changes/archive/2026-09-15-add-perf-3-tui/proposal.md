## Why

`add-perf-2-harness` built the sweep and left it mute. What `perftest kyoto` shows today is one line
per model to stderr — *"measuring Kyoto 4x (FP32)"* — and then nothing until the model is finished.
On the CPU that is a minute per run and six runs by default, so the tool that exists to tell you
what inference costs spends most of its life indistinguishable from a hang.

The silence is worse than cosmetic in one specific place, and it is the reason this slice is next
rather than optional. A model that is not on disk is downloaded **during the warm-up**, and a model
install reports itself through `ProcessOptions::on_progress` as `Stage::Installing` — the same
callback `add-perf-2-harness` deliberately leaves `None` on every run. `InitOptions::on_progress`,
which the harness *does* register, covers the ONNX Runtime and the GPU libraries at startup and
nothing else. So the first `perftest` of any model transfers several hundred megabytes with **no
report of any kind**, on a binary whose own README warns that a full sweep downloads gigabytes. The
progress reporting the library already composes — per operation, per pass, per tile, install folded
into the head of each operation's range — has never had a consumer.

This slice is that consumer, and it is the last of the seven the series was planned as.

## What Changes

- **An inline `ratatui` viewport, drawn on stderr while the sweep runs.** One managed line showing a
  spinner, the model being measured, its position in the sweep, the stage it is in (`warm-up 1/1`,
  `cold start`, `run 3/5`, or the install with its dependency named), a per-tile progress bar, the
  last timed run's duration and an estimate of the time left. The alternate screen is not used and
  no keyboard is read.

- **Finished models are written into the scrollback rather than into the view**, through
  `Terminal::insert_before`, so a sweep of twenty models leaves twenty lines on screen behind a view
  that is still one line tall — and they are still there after the program exits, which is the whole
  reason for an inline viewport rather than a full-screen one.

- **The per-tile bar is fed on every run, timed runs included.** `ProcessOptions::on_progress` is
  registered for the whole sweep, and on a run's own arm the callback does exactly one atomic store
  of a fraction and returns. Nothing is drawn from inside it: the renderer polls that atomic on its
  own timer, on its own thread, so what is inside the measured section is a store and not a terminal
  write. The install arm stores the dependency's name as well, which costs a lock and an allocation
  — and cannot be reached from a timed run, because a model is put on disk by the warm-up. This is
  the reference's decision and its reasoning, and it **refines `add-perf-2-harness`'s D6**, which
  had said no callback would be registered on a timed run at all — the rule that decision was
  protecting is that nothing *draws* during a measurement, and that rule is kept.

- **Every renderer is paced by the same ticker, including the two that draw nothing.** The sweep is
  polled beside a 15 fps ticker whatever renderer was chosen; under the viewport the tick draws,
  under `Lines` and `Silent` it fires a closure that does nothing. This is a measurement finding
  rather than a structural convenience, and it is the one place this slice changes how an
  *unwatched* sweep runs. A watched sweep measured about five percent **faster** than the same sweep
  under `--plain`, reproducibly on the CPU provider; deleting the callback from every timed run did
  not close the gap, so what remained was that one arm woke its task fifteen times a second and the
  other sat at an `await`. Giving both the same beat brought them to within 0.84%, and made the
  benchmark better at its job besides — both arms got faster and the run-to-run deviation fell three
  to six times. Weighed in `design.md` D10 with the figures.

- **An estimate of the time remaining, scoped to the model in flight.** Shown once at least one
  timed run has completed, from the median of the runs already taken plus the in-flight run's tile
  fraction. It deliberately does not extrapolate across models: their costs differ by more than an
  order of magnitude, so a sweep-wide figure would be a number the report would have to apologise
  for.

- **A renderer is chosen, not assumed.** The viewport when **both** stderr and stdout are a
  terminal; the existing per-model lines when either is not, when `--plain` is given, or when
  `--json` is. Stdout has to be tested as well as the stream the view draws on, which is a
  limitation found in implementation rather than a preference — placing an inline viewport means
  asking the terminal where the cursor is, and the layer that asks writes the question to stdout. It
  is weighed in `design.md` D7. The report itself — rendered or JSON, on stdout — is byte-for-byte
  the same whichever renderer ran.

- **`--plain` arrives**, which `add-perf-2-harness` listed as a non-goal *"because with one renderer
  there is nothing to opt out of"*. There are now two.

## Capabilities

### New Capabilities

None. Watching a sweep is not a new thing this project does; it is the benchmark capability gaining
the requirements about what it shows while it works.

### Modified Capabilities

- `performance-benchmark`: the capability today specifies what is measured, what the measurement is
  not allowed to quietly include, what the report must state, and how each outcome is accounted for
  — all properties of a sweep that has **finished**. What is added is the sweep in progress: that it
  reports what it is doing and how far through it is, that obtaining a model is visible rather than
  silent, that watching it cannot change what it measures or what the report says, that the display
  gives way to something a pipe can read when there is no terminal, and that what a sweep has
  already finished stays readable behind it. No existing requirement changes; these sit beside them.

## Impact

**Crates:** `crates/perf`, and nothing else. `opai`, `gui` and `cli` are untouched.

Nothing here passes the project's test for belonging in the library — *if both `gui` and `cli` would
need it, it belongs in `opai`*. A terminal viewport is a front end's own work, and the GUI's
equivalent is a React progress component fed by a Tauri event. What both front ends would need is
the progress *reporting*, and that is already in `opai`: `InferenceProgress`, `Stage` and
`OnInference` are public, `tests/public_api.rs` already names all three, and this slice is the first
thing to actually register one.

**Public API:** unchanged. Nothing is added to `opai`, nothing existing changes shape, and
`tests/public_api.rs` is untouched.

**rust-sak:** nothing, and nothing here duplicates a module. Its seven features are `crypto`,
`fetch`, `fs`, `image` (plus `image-raw`), `memo`, `o11y` and `sysinfo`; none covers terminal
rendering, and none is close. A TUI is not infrastructure another project would ask a first-party
utility crate for — it is presentation for one binary, which is the same conclusion
`add-perf-2-harness` reached about its summary table. TTY detection needs no crate at all:
`std::io::IsTerminal` asks the OS, which is exactly the test the reference had to reach for a
dependency to get right.

**New crates:** one. `ratatui` 0.30.2, with default features **off** and `crossterm` plus
`scrolling-regions` on — the latter is what lets `insert_before` push a finished model into the
scrollback without clearing and redrawing the viewport, which is otherwise a visible flicker once
per model. No spinner crate: `throbber-widgets-tui` exists and is maintained, but a spinner is an
array of frames indexed by a tick counter, and this is the same judgement `add-perf-2-harness` made
about `statrs` for five statistics. Weighed against `ratatui` in `design.md`.

**Behavioural divergences from the Go implementation, and why each is worth it:**

1. **The viewport is drawn on stderr, where every other renderer already writes.** The reference
   draws on stdout. Here stderr is already where the per-model lines and the install progress go,
   and `add-perf-2-harness` fixed that the report is the *whole* of stdout, so drawing there keeps
   that invariant without a special case: `perftest kyoto 2> /dev/null` correctly gets no view, and
   nothing of the display can ever reach the report.

   **What this does not buy is a watchable redirected sweep**, which this proposal originally
   claimed it would. `perftest kyoto > results.txt` falls back to the plain lines — the same
   outcome the reference has, so on that point there is no divergence to claim. The reason is in
   `design.md` D7: an inline viewport has to ask the terminal where the cursor is, and crossterm
   writes that query to `io::stdout()` regardless of which stream the backend draws on, so a
   redirected sweep put an escape sequence into the report file and stalled for two seconds waiting
   for an answer nobody could give. Testing stdout as well turns that into a clean, immediate choice
   of the plain renderer. The capability's requirement is still met — such a sweep reports a line
   per model on stderr while the file holds the report alone — and it was in fact *violated* before
   the fix, by the escape written into the file.

2. **`--verbose` does not force the plain renderer.** In the reference it must: `-v` installs a
   `slog` debug handler writing to stderr, which would tear a live view apart. Here the library's
   records go to a file — `opai::logging::init` is latched to one sink, which is the logging slice's
   decision — and `--verbose` only adds the per-run listing to the report that is printed after the
   sweep. There is nothing left for it to interfere with.

3. **An estimate of the time remaining, which the reference has not got**, scoped to the model in
   flight for the reason above.

4. **No keyboard handling and no raw mode.** The reference's Bubble Tea program owns the terminal
   and swallows key presses, Ctrl-C included, because it has to. Here Ctrl-C keeps reaching the
   signal handler `add-perf-2-harness` installed, which cancels the token the run already honours —
   so the sweep stops the same way whether or not anyone is watching, and there is no terminal state
   that a panic or a `kill -9` could leave behind.

**Estimated tasks:** 14, within the 15 the project allows, so this stays one change. The series is
listed in `design.md`; this is perf slice 3, change 7 of the seven, and the last of them.
