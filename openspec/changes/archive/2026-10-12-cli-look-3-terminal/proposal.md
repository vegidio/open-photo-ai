## Why

`opai` still draws its progress with `indicatif`: one plain `=>` bar per dependency, then one bar per library call, with
notes, a face report and provider details printed around them. The look decided for the CLI is MediaSim's: a short,
coloured layout around a single animated gradient bar. Slice 1 put that bar and its live session in `rust-sak`
(`26.10.6`). Slice 2 moved the detection-then-chain `Handover` into `opai` and gave every report its step and the
chain's length. This slice is the one the user sees: the CLI adopts both.

## What Changes

Task estimate: **13** (12 planned, plus 4.4 added during implementation for design D8). This is slice 3, the last of the `cli-look` series (see design.md - Slices).

The terminal layout on an interactive stderr, as the repository owner specified it:

```text
💾 Downloading dependency ONNX Runtime   34%                                    <- only when something is downloaded
🖼️ Applying enhancements to the image photo.jpg
                                                                                <- blank
Face Recovery   [1/2]  ███████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   38.4%
                                                                                <- blank
✨ Image enhanced successfully; file saved at out.avif                           <- or: 🧨 <short message>
```

- **The `💾` dependency line.** It is a single line, rewritten in place, and it appears only once a dependency is
  actually downloaded or expanded. The dependency's name is in yellow, followed by three spaces and the install's
  whole-number percentage in bold white. When the next dependency starts, its name and percentage replace the previous
  ones. Once initialization ends, the line stays frozen at the last dependency's `100%`. A machine with everything
  installed shows no line.
- **The `🖼️` header**: `Applying enhancements to the image <input>`, with the path in green.
- **One gradient bar for the whole run**, from `rust_sak::tui`. It covers the face detection and the chain through
  `opai::Handover`, so it moves forward once. The bar is labelled as the GUI labels its chip, in English:
  - `Enhancing` before the first report
  - `Downloading` while a model is being fetched, without the GUI's percentage
  - `Face Recovery` or `Upscale` (the family's short name), with a detection reading as `Face Recovery`

  It also has a `[n/N]` counter: `[0/N]` before the first report, then the reported step counted from one, where `N`
  is the planned chain's length. The line ends at the percentage, which is the progress of the whole run. There is no
  ETA. The label is padded to the widest label, so the bar does not shift.
- **The result line.** On success it reads `✨ Image enhanced successfully; file saved at <output>`, all in bold
  yellow. Every failure prints `🧨 <short message>` in bold red instead. That includes usage refusals the CLI makes
  itself, another instance holding the claim, and Ctrl-C. clap's own `--help`, `--version` and argument errors stay as
  clap prints them.
- **Off the terminal, into `opai.log`:** the skipped-upscale note, the face report (found/restoring/none/detection
  failed), the size, format and execution providers of the result, and `stopping…`. These are written as plain
  `tracing` records with no emoji. The library's own records are unchanged.
- **Plain output when stderr is not a terminal:** only the header and the result line are printed, without colour or
  live lines. `NO_COLOR` on a terminal only turns the colours off; the live lines are still drawn.
- **Ctrl-C under raw mode.** While a live line is up, Ctrl-C arrives as a key, not as SIGINT. During the bar it cancels
  the run, as it does today. During the dependency line it abandons initialization and exits with `130`, which is how
  the process ends today, minus the terminal being left in raw mode. In plain mode the SIGINT handler stays.
- **`indicatif` is removed.** `cli` names `ratatui` (for the dependency line's styled text and the colour of the plain
  lines) and `tracing` directly. Both are already in the lock file.
- **BREAKING (output only):** the terminal text changes completely. Nothing reads stdout (it stays empty), and exit
  statuses don't change. A script that parsed the old stderr messages (`wrote …; ran on …`, `found N faces`) loses them,
  and that information is in the log.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `cli-enhance`:
  - "Progress is shown while the command works" is rewritten for the new layout: the dependency line, the header, the
    single labelled bar with its counter, the result line, and plain output.
  - "The upscale factor defaults to what the photograph needs" and "Faces are found for a face recovery, and a failed
    detection does not stop the run": their notes and reports go to the log rather than the terminal.
  - "A run can be stopped" now covers Ctrl-C during the dependency download as well.
  - "Exit status says how the run ended": the one-line reason becomes the `🧨` line.

## Impact

- **Progress split (added during implementation, design D8):** `opai::Handover` now gives every enhancement an equal
  `1/N` of the bar, with a face detection inside the face recovery's slot rather than a fifth of the whole bar. This
  changes `opai` and the `face-recovery` spec, and what the GUI draws when its chain's detection runs.
- **Crates touched:** `cli`, in `Cargo.toml`, `main.rs`, `run.rs` and `progress.rs` (rewritten), plus a new `look.rs`
  for the coloured text lines. `opai`'s `Handover` (`inference/handover.rs`, its tests and the crate docs in `lib.rs`)
  for D8. In `gui`, only comments that describe the old split (`enhance/progress.rs`, `enhance/run.rs`,
  `useEnhancementRun.ts` and its test); no code. `perf` is not touched.
- **Logic in a front end:** what this slice adds is front-end presentation: the label table, the counter's display,
  colours, and choosing when to draw. Everything both front ends need was moved into `opai` by slice 2: the progress
  mapping, `Handover`, and the step index and count. The English labels repeat the GUI's English locale strings. They
  are presentation strings, as the GUI's i18n keys are, and not library vocabulary.
- **rust-sak:** uses `tui` (`26.10.6`, added in slice 1): `Live`, `LiveHandle`, `Target::Stderr`, `AnimatedProgress`
  and `impl LiveLine for Line<'static>`. Nothing new is needed there. Colour palettes, `paint`/`bold` and the
  `NO_COLOR`/`is_terminal` decision stay in each CLI, as slice 1 decided.
- **Dependencies:** `indicatif` is removed. `ratatui 0.30` (default features off, `crossterm`, as `perf` declares it)
  and `tracing 0.1` (as `opai` declares it) become direct dependencies of `cli`. tokio gains its `macros` feature for
  `select!`. No crate enters the lock file, and `indicatif` and its exclusive dependencies leave it.
- **Docs:** `main.rs`'s module docs describe the new output. The README's CLI section doesn't describe output and
  doesn't change.
