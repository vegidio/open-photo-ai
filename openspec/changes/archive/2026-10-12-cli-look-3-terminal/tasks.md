## 1. Dependencies

- [x] 1.1 In `crates/cli/Cargo.toml`, remove `indicatif`. Add `ratatui = { version = "0.30.2", default-features = false, features = ["crossterm"] }` (as `perf` declares it, minus `scrolling-regions`) and `tracing = "0.1.44"` (as `opai` declares it), each with a comment saying why `cli` names it. Add `macros` to tokio's features for `select!`. Verify with `cargo tree -p cli -i indicatif`, which finds nothing, and a `git diff Cargo.lock` that adds no crate

## 2. Text and labels (no terminal needed)

- [x] 2.1 Add `crates/cli/src/look.rs` with the following, ported in style from MediaSim's `output.rs` (design D4):
  - `use_color(is_terminal, no_color)` and the `Look { interactive, color }` decision
  - the palette constants and `paint`/`bold` over `ratatui::crossterm::style::Stylize`
  - `header(input)`, `success(output)` and `failure(error)`

  Verify unit tests cover `NO_COLOR` unset, empty and set; the exact uncoloured text of all three lines; the green path
  in the header; and bold yellow and bold red over the whole success and failure lines
- [x] 2.2 In `look.rs` (or `progress.rs`), add `dependency_line(name, fraction, color) -> Line<'static>` (design D2). Verify unit tests show the exact text `💾 Downloading dependency ONNX Runtime   34%`, yellow on the name and bold white on the percentage only with `color`, `0.999` drawing `99%`, and `1.0` drawing `100%`
- [x] 2.3 Add `label(report)`, `counter(report, planned)` and `LABEL_WIDTH` (design D3), with the family match total over `opai::Family`. Verify unit tests built from `InferenceProgress` literals cover every row of D3's table, including `[0/N]` before any report, a `Detection` report reading `Face Recovery` with the face recovery's position, an `Installing` report reading `Downloading`, and `LABEL_WIDTH == "Face Recovery".len()`

## 3. The live display

- [x] 3.1 Rewrite `crates/cli/src/progress.rs` around `rust_sak::tui` (design D1, D4). Its pieces:
  - The install callbacks start a `Live<Line<'static>>` on stderr lazily, on the first `Downloading`/`Extracting` report, and rewrite it on each report. `AlreadyInstalled` and unknown phases start nothing.
  - `finish_install()` freezes the line.
  - `start_bar(planned)` starts a `Live<AnimatedProgress>` with `[0/N]` and `Enhancing`, or nothing when `planned == 0`.
  - The bar's `OnInference` sets the label, counter and fraction through the handle. `finish_bar()` finishes the session and prints the blank line after it.
  - In plain mode every piece is a no-op. A failed `Live::start` is logged and leaves that line undrawn.

  Verify the module compiles with `indicatif` gone, and that unit tests drive the install callback with a sequence of `Progress` values on a non-interactive `Look`. They should show that no session is attempted and that the computed line text follows the reports, including the switch to the next dependency's name
- [x] 3.2 Wire Ctrl-C per design D5. Both sessions' `on_interrupt` handlers go through the view. During the install session they notify a `tokio::sync::Notify` that `run` selects against `Opai::initialize`. During the bar they cancel the run's `CancellationToken` and log `stopping…`. The SIGINT handler stays for plain mode. Verify with a unit test that the bar's interrupt cancels the token, and with the manual checks in 5.2

## 4. The run

- [x] 4.1 Rework `crates/cli/src/run.rs`: decide `Look` up front; run `Opai::initialize` under `select!` with the install interrupt, then `finish_install()`; print the header and the blank line; build `opai::Handover::new(&bar_callback, &plan.operations)` before the operations move into `find_chain_faces`; pass `handover.detection()` to `find_chain_faces` and `handover.chain()` to `process`; call `finish_bar()` on every outcome before returning; and print the success line after the write. Verify `cargo build -p cli` and that the existing `run.rs` tests still pass
- [x] 4.2 Move the off-terminal messages to `tracing` with no emoji, per the table in design D6: the plan's notes, `face_report`, `provenance` together with the size and format of the image written, `stopping…`, and the library-path warning in `main.rs`. Verify the existing `face_report`/`provenance` tests pass with their text unchanged apart from the dropped `note:`/`warning:` prefixes, and `rg 'say\(|eprintln!' crates/cli/src` lists only the layout's own lines and the log-sink warning
- [x] 4.3 In `crates/cli/src/main.rs`, print every `CliError` as `look::failure`, with the full error also recorded through `tracing::error!`. The blank line before it is printed only when a bar was drawn (design D7). Update the module docs for the new output. Verify `opai missing.jpg --up -o out.png` prints exactly one line, `🧨 …missing.jpg…`, and exits 2; that `opai photo.jpg --up --sharpen -o x.png` prints clap's own error with no `🧨` line; and that `opai --version` is unchanged

- [x] 4.4 In `opai::Handover`, give every enhancement an equal `1/N` slot of the bar and put a detection that ran into the first fifth of the first slot (design D8). Update its tests, the crate docs and the GUI comments that describe the old split, and add the `face-recovery` spec delta. Verify the handover tests show `--fr --up` as detection 0-10%, recovery 10-50% and upscale 50-100%, thirds for a three-operation chain with or without a detection, and a bar that never decreases

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace --locked`, and verify all three are green
- [x] 5.2 Manual checks on macOS with a copy of `fixtures/test.dat` renamed to `.jpg`, on both CoreML and CPU:
  - a first run with the runtime directory removed shows the `💾` line rising and frozen at `100%`, then the layout through `✨`
  - a second run shows no `💾` line
  - `--fr --up` shows `Face Recovery [1/2]` then `Upscale [2/2]`, with a bar that never refills, ending at `100.0%`
  - a run with a model removed shows `Downloading` with the step's counter
  - Ctrl-C during the bar ends with `🧨` and 130 and a usable shell
  - Ctrl-C during the `💾` line ends the same way, and the next run resumes the download
  - `2> log.txt` leaves only the header and the result line, with no escape sequences, and stdout is empty
  - `NO_COLOR=1` draws the live lines without colour
  - a 4000x3000 `--up` writes the file with no bar and logs the skip
  - a narrow terminal clips the bar rather than wrapping it
  - `opai.log` holds the moved messages, without emoji
- [x] 5.3 Repeat the layout, Ctrl-C and piped checks from 5.2 in Windows Terminal and in a Linux terminal emulator. For any platform not available, record it in this task as **untested**, and don't claim it
