## 1. The `tui` feature in `../rust-sak`

- [x] 1.1 Add the `tui` feature with `ratatui = { version = "0.30", default-features = false, features = ["crossterm"] }` to `../rust-sak/Cargo.toml`, and an empty `src/tui/mod.rs` behind `#[cfg(feature = "tui")]`; verify `cargo build --no-default-features --features tui` succeeds and `cargo build --no-default-features` compiles no `ratatui`

## 2. Widgets

- [x] 2.1 Port `Spring` (`src/tui/spring.rs`) and `GradientBar` (`src/tui/gradient_bar.rs`) from MediaSim's `crates/cli/src/progress/`, keeping their constants as they are; verify ported unit tests show the spring settling on its target and snapping exactly, the bar's first and last filled cells taking the gradient endpoints, and `color: false` writing no style
- [x] 2.2 Port the ETA formatter to `src/tui/eta.rs`; verify tests cover `--` for `None`, tenths below 10 s (`4.2s`), and MediaSim's longer forms
- [x] 2.3 Write `ProgressLine` in `src/tui/progress_line.rs` with `label_width`, an optional `[n/N]` counter, a caller-given percent floored to tenths, and the `Option<Option<Duration>>` ETA column (design.md - `ProgressLine` generalises MediaSim's widget); verify `TestBackend` tests reproduce every MediaSim `progress/mod.rs` layout test unchanged, and add tests showing a padded label keeps the bar's column fixed across labels of different lengths, a missing counter draws no brackets, a missing ETA column ends the line at the percent, and `0.9999` draws `99.9%`, not `100.0%`

## 3. The live session

- [x] 3.1 Write the `LiveLine` trait and its `Line<'static>` implementation in `src/tui/live_line.rs`, then `AnimatedProgress` (a spring plus `ProgressLine` fields, whose `settled()` is the spring's) in `src/tui/animated_progress.rs`; verify a test renders an `AnimatedProgress` frame by frame on `TestBackend` and sees the bar move monotonically towards a new target and report `settled()` once it arrives
- [x] 3.2 Write `Live` and `LiveHandle` in `src/tui/live.rs`: a `Target` of `Stdout` or `Stderr`, a terminal built by hand over the chosen stream with `Viewport::Inline(1)`, a 60 FPS frame thread, and `update` through a shared mutex; verify a manual run of an example on macOS with `Target::Stderr` still draws under `> /dev/null`, and that `Live::start` on a non-terminal returns an `io::Error` without leaving raw mode on
- [x] 3.3 Make Ctrl-C a key event that calls `on_interrupt` once per press while drawing continues (design.md - Ctrl-C is a key); verify `is_ctrl_c` unit tests for Ctrl-C and plain `c`, and verify manually that pressing Ctrl-C during the example calls the callback and leaves the process running
- [x] 3.4 Write `finish` (settle up to a configurable cap, default 1 s, then a final frame) and the guard whose `Drop` restores raw mode, moves the cursor below the line and shows it on every exit path; verify manually that after `finish`, after an early drop and after a panic in the producer the shell prompt comes back on a clean line with echo and line editing working

## 4. Documentation and CI

- [x] 4.1 Wire the public items into `src/tui/mod.rs` and `src/lib.rs`, write `src/tui/README.md` in the style of the other module READMEs (enabling, public API table, a stderr example, the no-other-writes rule, and that `NO_COLOR`/`is_terminal` are the caller's job), add a `tui` row to the crate README, and add `examples/tui.rs` with `required-features`; verify `cargo doc --no-default-features --features tui` renders the module and `cargo run --example tui --features tui` draws the line
- [x] 4.2 Add `tui` to the per-feature isolation matrix in `../rust-sak/.github/workflows/ci.yml`; verify the workflow file lists it, and that `cargo check --no-default-features --features tui --locked` passes locally

## 5. Release `rust-sak`

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `cargo test --all-features` and `cargo test --doc --all-features` in `../rust-sak`; verify all four are green
- [x] 5.2 **Get the repository owner's explicit go-ahead**, then push, bump the version to `26.10.5` and cut the tag; verify `gh api repos/vegidio/rust-sak/tags` lists `26.10.5` and the CI run on the tag is green on macOS, Windows and Linux. *(Released as `26.10.5`, then as `26.10.6` with the `scrolling-regions` fix described in design.md - Version and pins, both with the owner's go-ahead.)*

## 6. Adopt the pin here

- [x] 6.1 Move `crates/opai/Cargo.toml`'s `rust-sak` pin to `26.10.5`, and add `rust-sak = { git = "https://github.com/vegidio/rust-sak", tag = "26.10.5", features = ["tui"] }` to `crates/cli/Cargo.toml` with a comment saying why the CLI depends on it directly; update `Cargo.lock` and verify `cargo tree -p cli -i rust-sak` shows a single `rust-sak` and a cold `cargo build --locked` succeeds with no source change in this repository. *(Pinned to `26.10.6`, and `gui`'s and `perf`'s pins moved with it.)*
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace --locked`; verify all three are green and that `opai photo.jpg --up -o out.png` still draws the existing `indicatif` bars, since this slice deliberately does not change the CLI
