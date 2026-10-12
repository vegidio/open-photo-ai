## 1. Workspace foundation

- [x] 1.1 Create the root `Cargo.toml` as a virtual manifest: `[workspace]` with `resolver = "3"` and
      `members = ["opai", "cli"]`, a `[workspace.package]` block (version, `edition = "2024"`,
      authors, `license = "AGPL-3.0"`, repository) and the `[workspace.lints]` set from design D3
      (`unsafe_op_in_unsafe_fn = "deny"`; clippy `undocumented_unsafe_blocks`, `todo`,
      `unimplemented`, `dbg_macro` as warnings). Verify `cargo metadata --format-version 1 >
      /dev/null` succeeds and emits no resolver warning.

- [x] 1.2 Add `rustfmt.toml` copied verbatim from `../intercue/rustfmt.toml`, comment included.
      Verify with `diff ../intercue/rustfmt.toml rustfmt.toml` producing no output.

- [x] 1.3 Add `.gitignore` (at minimum `target/`, `.DS_Store`, `*.log`, editor directories) and
      `.editorconfig` per design D6 - UTF-8, 4-space, 120 columns, trimmed trailing whitespace,
      2-space YAML, explicit `[*.rs]`, `insert_final_newline = true`, no `[*.go]` stanza. Verify
      `cargo build` leaves `git status --porcelain` empty.

## 2. Crates

- [x] 2.1 Create the `opai` library crate with no dependencies (design D4): `Cargo.toml` inheriting
      the workspace package fields and `lints.workspace = true`, and `src/lib.rs` exposing one public
      function returning the application version, with its test colocated in the same file. Verify
      `cargo test -p opai` passes.

- [x] 2.2 Create the `cli` binary crate depending only on `opai`, inheriting the same workspace
      fields and lints, with `src/main.rs` holding nothing but a `main` that calls the `opai`
      function and prints the result. Verify `cargo run -p cli` prints the version and exits 0.

## 3. Repository files

- [x] 3.1 Add `LICENSE` with the AGPL-3.0 text, matching the Go project's (design D6). Verify
      `diff ../open-photo-ai/LICENSE LICENSE` produces no output and that the `license` field in
      `[workspace.package]` reads `AGPL-3.0`.

- [x] 3.2 Add a `README.md` stub: what the project is, the `opai` / `cli` / `gui` layering with one
      line each, and the developer commands that exist today (`cargo run -p cli`, `cargo test`).
      Note that `gui` arrives in `add-scaffold-2-gui`. Verify by reading it back - a newcomer should
      be able to build and run the workspace from it alone.

## 4. Continuous integration and verification

- [x] 4.1 Add `.github/workflows/ci.yml` per design D5: triggers on push to `main`, pull request and
      `workflow_dispatch`; `permissions: contents: read`; a `concurrency` group cancelling superseded
      runs; and three `ubuntu-latest` jobs - `fmt` (`cargo fmt --all --check`), `clippy`
      (`cargo clippy --all-targets --locked -- -D warnings`, with `Swatinem/rust-cache`) and `test`
      (`cargo test --locked`). Verify with `actionlint` if available, otherwise by confirming the
      workflow parses as valid YAML and each job's `run` commands match those already passing in 4.2.

- [x] 4.2 Run the full local gate and commit `Cargo.lock`: `cargo fmt --all --check`, `cargo clippy
      --all-targets --locked -- -D warnings` and `cargo test --locked` must all pass with the lock
      file tracked by git, since CI's `--locked` depends on it. Verify all three commands exit 0 and
      `git ls-files Cargo.lock` lists the file.
