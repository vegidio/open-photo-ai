## 1. The spinner in `../rust-sak`

- [x] 1.1 Add `src/tui/spinner.rs` with `Direction` (`Clockwise` the default, `CounterClockwise`) and `Spinner` (`direction`, `style` defaulting to `#ee6ff8`, `color`, a private `started: Instant`; `new`, `frame_at`, `glyph`, `span`, and `Widget for &Spinner`), holding the single clockwise frame table `⣾ ⣷ ⣯ ⣟ ⡿ ⢿ ⣻ ⣽` read backwards for `CounterClockwise` (design.md - One frame table; the direction reads it forwards or backwards); export both from `src/tui/mod.rs`; verify `cargo build --no-default-features --features tui` succeeds
- [x] 1.2 Add unit tests: `frame_at` over 0..800 ms in 100 ms steps gives exactly `⣾ ⣷ ⣯ ⣟ ⡿ ⢿ ⣻ ⣽` clockwise and `⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷` (bubbles' `spinner.Dot`) counter-clockwise; 799 ms is still frame 7 and 800 ms wraps to `⣾`; `span` carries `style` only when `color` is on; drawing `&Spinner` into a 1x1 `TestBackend` buffer writes the glyph; verify `cargo test --no-default-features --features tui spinner` is green

## 2. Documentation and example

- [x] 2.1 Add `Spinner` and `Direction` rows to `src/tui/README.md`'s API table, with a short paragraph on the time-based frame (10 per second, independent of the draw rate) and on embedding `span()` in a `Line`; verify `cargo doc --no-default-features --features tui` renders the section without warnings
- [x] 2.2 Add `examples/tui_spinner.rs` (a `Live` session drawing `Downloading… <spinner> <n>% Complete` for a few seconds, then the same line counter-clockwise), registered in `Cargo.toml` with `required-features = ["tui"]`; verify by hand on macOS that `cargo run --example tui_spinner --features tui` turns clockwise first, then counter-clockwise, at a steady speed

## 3. Release `rust-sak`

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `cargo test --all-features` and `cargo test --doc --all-features` in `../rust-sak`; verify all four are green
- [x] 3.2 Keep the version at `26.10.6` and, with the repository owner's explicit go-ahead, commit, push, delete the `26.10.6` tag and recreate it on the new commit (the owner chose to move the tag instead of releasing `26.10.7`); verify `git ls-remote --tags origin 26.10.6` points at the new commit and CI on it is green on macOS, Windows and Linux

## 4. Adopt the pin here

- [x] 4.1 Keep `tag = "26.10.6"` in `crates/opai/Cargo.toml`, `crates/cli/Cargo.toml`, `crates/gui/Cargo.toml` and `crates/perf/Cargo.toml` and run `cargo update -p rust-sak` so the lock file follows the moved tag; verify `cargo tree -i rust-sak --workspace` shows a single `rust-sak` at the new commit and `cargo build --workspace --locked` succeeds with no source change
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace --locked`; verify all three are green
- [x] 4.3 Verify by hand that `opai <photo>.jpg --up -o out.png` draws its `💾` line, bar and list exactly as before, since this change deliberately does not change the CLI
