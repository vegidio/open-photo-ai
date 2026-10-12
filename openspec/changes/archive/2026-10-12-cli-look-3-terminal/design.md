## Context

See proposal.md - Why. The layout itself is the repository owner's specification, recorded in the exploration that
started this series. The requirements are in `specs/cli-enhance/spec.md`. These facts, checked in the code, shape how
the CLI gets there:

- **Today's `cli` draws two bar groups with `indicatif`** (`crates/cli/src/progress.rs`). `View::install` adds one bar
  per planned dependency. `View::inference` adds one bar per library call: the detection gets one bar, which is
  cleared, and then the chain gets another. `View::say` prints notes, the face report, `stopping…` and the completion
  line above the bars. `run.rs` builds both `on_progress` callbacks separately and installs the `tokio::signal::ctrl_c`
  handler only after `Opai::initialize` returns.
- **Slice 2 left everything the bar needs on the report.** `opai::Handover::new(&on_progress, &operations)` gives a
  `detection()` and a `chain()` callback that report into one `0..1` figure. Every `InferenceProgress` carries
  `operation_index` (zero-based, apply order) and `operation_count`, and a detection report carries the face recovery's
  position. `report.subject.family()` names the family, and a detection reports `Family::Detection`.
- **Slice 1's `rust_sak::tui` (`26.10.6`, already pinned in `cli`)** gives `Live::start(Target::Stderr, line,
  on_interrupt)`, a `LiveHandle::update`, and `finish()`, which settles the line, leaves the last frame in the scrollback
  and puts the cursor below it. `AnimatedProgress` has `label`, `label_width`, `counter`, `fraction`, `eta` and `color`.
  A plain `ratatui::text::Line<'static>` is a `LiveLine`. Its rules: nothing else may write to stderr while a session
  is up, and `Live::start` on a non-terminal returns `Unsupported` rather than falling back. Ctrl-C arrives as a key in
  raw mode, so SIGINT no longer fires.
- **Install progress** (`opai::Progress`): the library installs dependencies one at a time, in plan order. Each report
  has `dependency.as_str()` (`ONNX Runtime`, `NVIDIA CUDA`, …), a `phase` (`Downloading`, `Extracting`,
  `AlreadyInstalled`, `#[non_exhaustive]`) and a `fraction` over that dependency's whole install, which ends at exactly
  `1.0`. `on_plan` lists installed dependencies too, so it cannot decide whether a line is needed.
- **`Opai::initialize` has no cancellation.** Today a Ctrl-C during it kills the process by default. The single-instance
  claim is released by the system, and a download resumes from the bytes on disk.
- **`cli` has no `tracing` dependency.** `opai::logging::init` has already installed the `opai.log` sink, so a
  `tracing` record from `cli` lands in the same file.
- **The GUI's English labels** (`preview.progress` in `en.json`): `Enhancing`, `Face Recovery`, `Upscale`, `Denoise`,
  `Colorize`, `Light Adjust`, `Color Balance`, `Sharpen`, and `Downloading {{percent}}%`.

## Slices

The CLI's new look is delivered as three changes. **This change is slice 3.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-look-1-rust-sak-tui` | `rust-sak`'s `tui` feature: `GradientBar`, `Spring`, `ProgressLine`, the ETA text, and the live one-line session that writes to stdout or stderr. A `rust-sak` release (`26.10.6`). Here: the `rust-sak` pins moved to the new tag, and `cli` depending on `rust-sak` with `tui` | - | DONE (archived 2026-10-12) |
| 2 | `cli-look-2-progress-core` | In `opai`: the detection-then-chain `Handover` moved from `gui` into the library, and the operation's index and the chain's length carried on `InferenceProgress`. The GUI's behaviour doesn't change | - | DONE (archived 2026-10-12) |
| 3 | `cli-look-3-terminal` | In `cli`: the new layout: the `💾` dependency line frozen at the last dependency's 100%, the `🖼️` header, the labelled gradient bar using the GUI's labels, and the `✨`/`🧨` result line. Also: `indicatif` removed; notes, face report and provider details moved to the log; plain output when stderr is not a terminal; and the `cli-enhance` spec delta | 1, 2 | **CURRENT** |

This slice ends the series. Moving MediaSim onto `rust_sak::tui` happens in MediaSim's own repository and is outside
it.

## Goals / Non-Goals

**Goals:**

- The layout in proposal.md, exactly as specified, on stderr, with stdout left empty.
- The terminal is restored on every way a run ends: success, failure, Ctrl-C during either live line, and a panic.
- Every piece of text and every labelling rule is testable without a terminal.

**Non-Goals:**

- Any change to `gui`, `perf` or `rust-sak`, and any change to `opai` beyond D8's mapping in `Handover`.
- New flags, such as `--quiet`, `--plain` or `--color`. `NO_COLOR` and `is_terminal` are the only switches, as in
  MediaSim.
- Cancelling `Opai::initialize` cleanly from inside the library. See D5.
- Translating the CLI. The labels are the GUI's English strings.

## How the work divides

- **`cli`**: all of it. That covers the text lines and colours (`look.rs`), the live display and its two sessions
  (`progress.rs`, rewritten), the wiring and the moved messages (`run.rs`), and the failure line (`main.rs`).
- **`opai`**: only D8's mapping in `Handover` (`inference/handover.rs`, its tests and the crate docs in `lib.rs`).
  Everything else both front ends need (the chain fraction, `Handover`, and the step index and count) is already
  there. What this slice adds otherwise is presentation: which words, which colours, and whether to draw at all. This
  is the same work the GUI does in `ProgressBar.tsx` and `lib/enhancements.ts`.
- **`gui`**: no code. Only the comments that describe the old split change (`enhance/progress.rs`, `enhance/run.rs`,
  `useEnhancementRun.ts` and its test), since D8 changes what the GUI's bar draws when its chain's detection runs.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Gradient bar, label padding, counter, percent, no ETA | `AnimatedProgress` (`eta: None`) | `tui` |
| Live line on stderr, Ctrl-C as a key, restore on every exit | `Live`, `LiveHandle`, `Target::Stderr` | `tui` |
| The dependency line, a styled line rewritten in place | `impl LiveLine for Line<'static>` | `tui` |
| Colours of the header, success and failure lines; `NO_COLOR` | **local** (`look.rs`) | - |

The last row stays local, as slice 1 decided. Palettes and the decision whether to colour belong to each CLI, and
MediaSim keeps its own `paint`/`bold`/`use_color` too. These are a few lines, and both CLIs choose different colours
for them.

## Decisions

### D1. Two live sessions, with plain lines printed between them

```text
 initialize ──► [Live<Line> "💾 …"]  (started lazily on the first Downloading/Extracting report)
                       │ finish() when initialize returns: frozen frame stays, cursor below
 eprintln "🖼️ Applying enhancements to the image <input>"
 eprintln ""
 find_chain_faces + process ──► [Live<AnimatedProgress>]  via Handover.detection() / .chain()
                       │ finish() when process returns, whether it succeeded or failed
 eprintln ""
 eprintln "✨ …"  or  "🧨 …"
```

`Live` draws one row and must have the stream to itself, so the plain lines are printed only when no session is up.
That is the order above. A single session over the whole run would have to draw a multi-line viewport, which slice 1
deliberately left out.

**The dependency session starts lazily.** `on_plan` can't tell whether anything will be transferred, because it also
lists installed dependencies. So the `OnProgress` callback starts the session on the first report whose phase is
`Downloading` or `Extracting`. `AlreadyInstalled`, and any phase this crate doesn't know (`Phase` is
`#[non_exhaustive]`), start nothing. The callback runs on library threads, so the session sits behind a
`Mutex<Option<Live<Line>>>` owned by the view. `start` happens at most once, and `finish` takes it out after
`initialize` returns.

**Alternative rejected: print the header first and the dependency line below it.** The owner's layout puts `💾` first.

### D2. The dependency line's text

`Line::from(vec!["💾 Downloading dependency ", name (yellow), "   ", "<p>%" (bold white)])`, rebuilt on every report.
`p = floor(fraction × 100)`, so `100` appears only when the library's final report lands on exactly `1.0`. That same
report is what leaves the line "frozen at the last dependency's 100%". The text stays "Downloading" during the
expansion: the owner asked for one line that changes only its name and number, and the percentage already covers both
phases.

### D3. The bar's label and counter are a pure function of the report

```rust
fn label(report: Option<&InferenceProgress>) -> &'static str   // Enhancing | Downloading | the family's short name
fn counter(report: Option<&InferenceProgress>, planned: usize) -> (usize, usize)  // (0, N) | (index + 1, count)
```

| Report | Label | Counter |
|---|---|---|
| none yet | `Enhancing` | `[0/N]`, `N` = `faced.operations.len()` |
| `Stage::Installing(_)` | `Downloading` | `[index+1/count]` |
| `Running` / `Cached`, family `FaceRecovery` or `Detection` | `Face Recovery` | `[index+1/count]` |
| `Running` / `Cached`, family `Upscale` | `Upscale` | `[index+1/count]` |
| other families | the GUI's English short name (`Denoise`, `Colorize`, `Light Adjust`, `Color Balance`, `Sharpen`) | `[index+1/count]` |
| a stage this crate doesn't know | `Enhancing` | `[index+1/count]` |

`Family` isn't `#[non_exhaustive]`, so the family match is total and a new family fails to compile here until it is
given a label. The CLI can only request two families, but `cli` names all seven, so the table can't silently disagree
with the GUI's. `label_width` is the widest entry in this table (`Face Recovery`, 13). `fraction` is the
`chain_fraction` coming through `Handover`, and `eta` is `None`. These are the GUI's rules from `ProgressBar.tsx`, with
one exception: `Downloading` drops its percentage, at the owner's direction.

`N` is `faced.operations.len()`, read before the bar starts. It is the same chain `process` runs, so it equals every
report's `operation_count`. A skipped upscale has already been removed from it. Where it is `0`, no bar session is
started (spec: "no progress bar SHALL be drawn"), since `[0/0]` with an empty bar says nothing.

**Alternative rejected: map families to labels with `display_name()`.** That gives `Athens (FP32)`, the library's
sentence for the log, not the GUI's chip label.

### D4. Interactive, colour and plain are decided once, up front

`Look { interactive: stderr.is_terminal(), color: interactive && NO_COLOR is unset or empty }`, decided in `run`
before anything is printed, as MediaSim's `use_color(is_terminal, no_color)` is. That is a pure function, and it is
unit-tested.

- **interactive**: the live sessions are used, and blank lines frame the bar.
- **plain** (`!interactive`): no session is started, no blank lines are printed, and only the header and the result
  line are written. The callbacks still exist, but they do nothing, so the run's code path is the same.
- **colour** goes to the plain lines through `paint`/`bold` helpers on `ratatui::crossterm::style::Stylize`. That is
  MediaSim's approach, reached through `ratatui`'s re-export, so no direct `crossterm` dependency is needed. It goes to
  the live lines as `AnimatedProgress::color` and as styles on the `Line`'s spans.

If `Live::start` fails on what reported itself as a terminal (for example, a cursor-position query that times out),
that line is skipped and the failure is logged. The run continues, and the plain lines are still printed. Losing
progress is never a reason to fail a run.

Palette (MediaSim's constants): green `#00c202` for the input path, yellow `#e5db08` for the dependency name and the
success line, and red `#f44336` for the failure line. The dependency's percentage is bold `Color::White`.

### D5. Ctrl-C in each phase

| Phase | Ctrl-C arrives as | Action |
|---|---|---|
| before `initialize`, or during it with nothing downloading (no session up) | SIGINT, default handler | the process ends, as today |
| the `💾` session is up | a key, via `on_interrupt` | notify a `tokio::sync::Notify`; `run` `select!`s on it against `initialize`; the `initialize` future is dropped, the session is finished (restoring the terminal), and `CliError::Cancelled` follows (`🧨`, 130) |
| the bar session is up | a key, via `on_interrupt` | `cancel.cancel()` and log `stopping…`; the bar keeps drawing until the library returns `Cancelled` |
| plain mode, after `initialize` | SIGINT, via `tokio::signal::ctrl_c` | `cancel.cancel()`, as today |

The SIGINT handler from today stays for plain mode and for the gaps between sessions. While a session is up, raw mode
means SIGINT is never delivered, so the two paths never both fire for one press.

**Dropping `initialize` mid-download is no less safe than today's outcome.** Today the process is killed at the same
point. Dropping the future stops at an await point, and the process then exits within milliseconds. Both leave
whatever the installer leaves on an abrupt end: a partial download that resumes, and a claim that is released.
Dropping releases the claim through its guard, not through process exit.

**Alternative rejected: exit from inside `on_interrupt`.** That runs on the frame thread, and `process::exit` would
skip the guard that restores the terminal.

**Alternative rejected: a cancellation token on `InitOptions`.** That is an `opai` change, which this slice doesn't
need. A real cancellation of `initialize` is a library feature that both front ends could use, so it belongs in its
own change.

### D6. The moved messages become `tracing` records in `cli`

| Was on the terminal | Now |
|---|---|
| `note: the upscale was skipped …` | `tracing::info!` with the dimensions |
| `found N faces, restoring M` / `no faces were found …` | `tracing::info!` |
| `warning: face detection failed …` | `tracing::warn!` carrying the error |
| `wrote <path> (WxH, format); ran on …` / `served from cache` / `no enhancement ran` | `tracing::info!` |
| `stopping…` | `tracing::info!` |
| every failure (also shown as `🧨`) | `tracing::error!` with the full error |
| `could not establish the library search path …` | `tracing::warn!` (the log is up by then) |

The text of each record is the existing message minus its terminal-only prefix (`note:`, `warning:`), with no emoji,
as the owner asked. `face_report` and `provenance` keep their logic and tests and just change their destination.
`could not install the log sink` stays on stderr, because the log is what failed. It is printed before any session
exists, so it breaks no rule.

### D7. The failure line

`main` prints `🧨 <error>` in bold red for every `CliError`, using its existing one-line `Display` (the "short
message"). After a bar, the layout's own blank line comes first, so the failure line sits where the success line would.
With no bar drawn, nothing comes before it, since the dependency line and the header already end with a newline.
clap's errors, `--help` and
`--version` end in `args::parse` before any of this, so they are unchanged.

**Tracking whether a blank line is owed.** The view owns it. `View::finish_bar`, which `run` calls on every outcome
before returning, prints the blank line only when a bar session was actually drawn. `main` then prints the `🧨` line
with nothing before it, so a failure before the bar (usage, the claim, initialization) prints one `🧨` line and no stray
blank line, and `main` never has to know whether a bar existed.

### D8. Every enhancement owns an equal slot, and the detection is inside one

Found while checking the bar by hand: `Handover` gave a detection that ran the first fifth of the **whole** bar and
spread the chain over the rest, so `--fr --up` drew face recovery over 0-60% and the upscale over 60-100%. The GUI used
the same mapping before slice 2 moved it into `opai`; it only looked like 0-50% there because the picker had usually
found the faces already, and a detection the store serves takes no share.

The owner's rule: every enhancement owns `100% / N` of the bar, and a face recovery is one enhancement, its detection
included. So `Handover` now keeps the chain's equal `1/N` slots and puts a detection that ran into the first fifth of
the **first slot**, with the first operation's own reports squeezed into the rest of it. With the face recovery applied
first, which is every CLI chain and most GUI ones, that is exactly the face recovery's slot: 0-10% detection, 10-50%
recovery, 50-100% upscale. A detection the store served still reports nothing, and the chain is then forwarded as it is.

A face recovery applied after another enhancement (a denoise, in the GUI) cannot hold its own detection: the detection
runs before the whole chain, and the bar only moves forward. There the detection sits in the first operation's slot,
and every slot stays `1/N`.

That case has one visible quirk, accepted on purpose. The detection's reports carry the face recovery's position, so
during the head of the first slot the bar reads `Face Recovery [2/2]`, and then `Denoise [1/2]` once the chain starts:
the counter steps back while the bar keeps moving forward. Attributing the detection to its face recovery is what the
spec asks, and the alternative, a detection labelled as a denoise, would be wrong. Only the GUI can build such a chain,
since the CLI always applies the face recovery first.

This changes `opai` (the mapping, its tests, the crate docs) and the `face-recovery` spec, and changes what the GUI
draws when its chain's detection does work. The GUI's code is untouched apart from comments.

**Alternative rejected: remap in `cli` only.** It would duplicate the progress mapping in a front end and leave the two
front ends disagreeing, which the layering rules forbid.

## Risks / Trade-offs

- **[Emoji width varies between terminals.]** `🖼️` carries a variation selector, and some terminals draw it one
  column wide. → It sits only in plain lines printed with `eprintln`, never in a viewport. `💾` is inside the
  dependency session's `Line`, where ratatui measures it with `unicode-width` (width 2). If a terminal disagrees, the
  three spaces before the percentage may look like two. This is accepted, and it is checked by hand in the manual
  task.
- **[A library or native write to stderr during a session lands on the line.]** → `opai`'s records go to `opai.log`,
  and ONNX Runtime's native logging is routed through `tracing` (`add-logging-1-file`). `cli` prints only between
  sessions (D1). Any stray line found by the manual checks is a bug to fix where it is written.
- **[Dropping `initialize` at an arbitrary await point.]** (D5) → It is no worse than the kill it replaces. The manual
  task interrupts a first-run download and checks that the next run resumes it and that the claim is free.
- **[Windows and Linux terminals are untested by slice 1.]** → This slice's manual checks name them. Any terminal not
  available to the implementer is recorded as untested in the task, not claimed.
- **[Information leaves the terminal.]** A user who relied on seeing the provider or the face count now has to open
  `opai.log`. → This is the owner's decision. The log path is the one the GUI's "show logs" opens.

## Validation

- Platforms: the text builders, `label`/`counter` and `use_color` are unit-tested and run in workspace CI on macOS,
  Windows and Linux. The live layout is checked by hand on macOS (Terminal and a second terminal emulator). Windows
  Terminal and a Linux terminal are checked by hand if available, and recorded as untested otherwise.
- Execution providers: the manual run covers CoreML and CPU on macOS. CUDA and TensorRT are **untested** here. Nothing
  in this slice depends on the provider, but the first-run dependency line for `NVIDIA CUDA`, `cuDNN` and `TensorRT` can
  only be seen on an NVIDIA machine.

## Migration Plan

This is one commit, in `cli` plus D8's `Handover` mapping in `opai` and the GUI comments that describe it. `Cargo.lock` loses `indicatif` and whatever only it used, and gains no crate. Rollback is
reverting the commit. No data, settings, cache keys or wire shapes change.
