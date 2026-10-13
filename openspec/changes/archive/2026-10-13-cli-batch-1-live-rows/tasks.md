## 1. Multi-row sessions in `../rust-sak`

- [x] 1.1 Add `fn height(&self) -> u16 { 1 }` to `LiveLine` in `src/tui/live_line.rs`, documented as read once when the session starts; verify every existing `LiveLine` implementation compiles unchanged and a unit test shows `AnimatedProgress` and `Line<'static>` report `1`
- [x] 1.2 Make `Live::start` read `line.height()` before the line moves into the mutex and pass `Viewport::Inline(max(height, 1))` to `Session::start` (design.md - The height is a `LiveLine` method, read once at start); verify a test with a 3-row `LiveLine` on `TestBackend` sees a 3-row `Rect` in `render`, and a height of 0 gets 1 row
- [x] 1.3 Change `Session`'s `Drop` to restore below the viewport's bottom row (`area().bottom() - 1`) while keeping the `max` with the reported cursor row (design.md - The guard restores below the viewport's bottom row); verify by hand on macOS that after `finish`, after an early drop and after a panic in the producer, a 1-row and a 7-row session both leave the prompt on a clean line below the drawing
- [x] 1.4 Verify by hand on macOS that a 7-row session started on a 4-row window draws clipped into 4 rows and restores cleanly, and that one started on the window's last row scrolls the terminal up without overwriting the scrollback

## 2. Documentation and example

- [x] 2.1 Add `height` to the `LiveLine` row of `src/tui/README.md`'s API table, with a paragraph on fixed height, the clamped `Rect` and blank rows; add `examples/tui_rows.rs` (a bar over a five-row list that scrolls to the most recent item, split with ratatui's `Layout`) with `required-features = ["tui"]`; verify `cargo doc --no-default-features --features tui` renders it and `cargo run --example tui_rows --features tui` draws it

## 3. Release `rust-sak`

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `cargo test --all-features` and `cargo test --doc --all-features` in `../rust-sak`; verify all four are green
- [x] 3.2 **Get the repository owner's explicit go-ahead**, then commit and move the existing `26.10.6` tag to that commit, keeping the version at `26.10.6` (design.md - The release reuses the `26.10.6` tag); verify `gh api repos/vegidio/rust-sak/tags` lists `26.10.6` at the new commit and the CI run on the tag is green on macOS, Windows and Linux

## 4. Adopt the pin here

- [x] 4.1 Keep `tag = "26.10.6"` in `crates/opai/Cargo.toml`, `crates/cli/Cargo.toml`, `crates/gui/Cargo.toml` and `crates/perf/Cargo.toml`, and move only the `rust-sak` and `o11y-macros` commit hashes in `Cargo.lock` to the retagged commit; verify `cargo tree -i rust-sak --workspace` shows a single `rust-sak` and `cargo build --workspace --locked` succeeds with no source change
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace --locked`; verify all three are green
- [x] 4.3 Verify by hand that `opai <photo>.jpg --up -o out.png` draws its one-row bar exactly as before and leaves the prompt on the line below the result line, since this change deliberately does not change the CLI
