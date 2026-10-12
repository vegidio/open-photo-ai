## 1. The seam and the choice

- [x] 1.1 Add `ratatui` 0.30.2 to `crates/perf` with `default-features = false` and
      `features = ["crossterm", "scrolling-regions"]`, and add the `--plain` flag to `Options`.
      Verify `cargo build -p perf` succeeds, that `perftest --help` lists `--plain`, and by unit
      test that the renderer choice is the viewport only when stderr is a terminal and neither
      `--plain` nor `--json` was given — so that `--json` and a redirected stderr both still resolve
      to the renderer they resolve to today (D1, D7).
- [x] 1.2 Widen `sweep::Listener` with a `stage` method carrying `Warmup { run, of }`, `Cold`,
      `Timed { run, of }` and `Run { elapsed }`, called from the four points the loop already has;
      give `Silent` an empty body and have `Lines` ignore it. Verify by unit test against a
      recording listener that the four stages arrive in loop order with their counters, and by
      reading the diff that nothing else in `sweep.rs` moved — not a release, not an identity, not
      the `Enhanced` drop, not the classification (D3).

## 2. What the sweep reports while it runs

- [x] 2.1 Register `ProcessOptions::on_progress` on every run — warm-up, cold start and timed runs
      alike — with a callback that stores `chain_fraction` into an `AtomicU64` as bits and, for
      `Stage::Installing`, the dependency's name and byte counts, and returns. Verify by unit test
      that the callback writes and does nothing else, and by reading it that it performs no I/O, no
      allocation and no lock of this crate's (D2).
- [x] 2.2 Build the shared view state — the model and its position, the stage, the last completed
      run, the timed runs taken so far — written by the listener under a mutex that is never held
      across terminal I/O, and read by the renderer beside the atomics of 2.1. Verify by unit test
      that a sequence of listener calls leaves the state a frame would be composed from, and that
      the state is reset when a model starts so nothing of the previous model leaks into the next
      (D4).

## 3. The viewport

- [x] 3.1 Compose the active line as a pure function of the view state and a terminal width: spinner
      frame, codename at a fixed width, position in the sweep, stage label, bar, and the tail each
      stage has — the last run and the estimate during the timed runs, the dependency and its byte
      counts during an install. Verify by unit test, with no terminal, that each stage is named,
      that durations render through `report::duration` so a watched figure and a table figure are
      spelled identically, that a narrow width drops the estimate first, then the last run, then the
      bar, and that the result never exceeds the width and never wraps (D9).
- [x] 3.2 Compute the estimate over the current model's remaining timed runs, from the median of the
      runs already taken plus the in-flight fraction, excluding the cold start. Verify by unit test
      that nothing is offered before the first timed run completes, that it falls as runs complete,
      that it covers only the model in flight, and that it is absent during the warm-up, the cold
      start and an install (D8, and the estimate requirement).
- [x] 3.3 Drive the viewport from a `select!` over the pinned sweep future and a 15 fps ticker in the
      sweep's own task, on stderr, through `Viewport::Inline(1)`. Verify by running a sweep that the
      spinner turns and the bar advances during a model's runs, and that nothing is spawned and no
      borrow in `sweep::sweep`'s signature had to become `'static` (D4).
- [x] 3.4 Write each finished model into the scrollback with `Terminal::insert_before` — the model,
      its outcome, and its cold start and median or the reason it was declined, failed or
      interrupted. Verify by running a sweep of several models that the view stays one line tall,
      that the finished lines do not flicker as later models are measured, and that they are still
      on screen after the process exits (D5, and the requirement that what a sweep has finished
      stays readable).
- [x] 3.5 Hide the cursor behind a guard whose `Drop` restores it, with a panic hook alongside it,
      and enter neither raw mode nor the alternate screen. Verify by interrupting a sweep, and by
      panicking one deliberately, that the shell is left with a visible cursor and a working echo,
      and that Ctrl-C still cancels the run in flight and prints the summary as it does today (D6).

## 4. Proving it did not change the measurement

- [x] 4.1 Verify by unit test, with no terminal, that the report is unchanged by which renderer ran:
      the same `SweepResult` rendered after a viewport sweep and after a plain one produces
      byte-identical output from `report::body` and from `json::render`, and that a viewport is never
      chosen under `--json` (the watching-does-not-change-it requirement, D7).
- [x] 4.2 Measure the cost of 2.1 by hand: the same model at a high run count, once watched and once
      under `--plain`, on macOS arm64 against CoreML and against the CPU. Verify that the medians sit
      inside each other's run-to-run spread. **If they do not, D2 was wrong** — record the figures,
      move the callback registration outside the timed sections, keep it on the warm-up where the
      install reporting lives, and say so in the README (D2, and the risk it carries).
- [x] 4.3 Verify by running it that a model which is not on disk reports its transfer while it
      happens, naming the model and its byte counts, rather than the several minutes of silence the
      harness shows today — and that this is inside the warm-up, so no timing covers it (the
      obtaining-a-model requirement).

## 5. Documentation and checks

- [x] 5.1 Update `crates/perf/README.md`: what the live view shows and what each part of the line
      means, `--plain` and when it is chosen automatically, that the view is drawn on stderr so a
      redirected report stays watchable, what the estimate covers and what it deliberately does not,
      and what 4.2 measured about the callback's cost. Verify every command it shows runs as written.
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace and verify
      all three are clean.
