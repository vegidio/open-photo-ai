## 1. Release `rust-sak`'s `github` module

- [x] 1.1 In `../rust-sak`, confirm `main` is clean and `github` is covered: run `cargo test --features github`; verify
      the module's tests pass on the commit about to be tagged
- [x] 1.2 Bump `../rust-sak/Cargo.toml`'s version to `26.9.8`, commit it as the repository does
      (`chore: Bump the version to 26.9.8`) and tag `26.9.8`; with the user's go-ahead confirmed at this point, push the
      commit and the tag; verify `git ls-remote --tags https://github.com/vegidio/rust-sak` lists `26.9.8`
- [x] 1.3 Move the `rust-sak` pin to `26.9.8` in `opai`, `gui` and `perf`, add `github` to `gui`'s features, and
      refresh the lock with `cargo metadata`; verify the lock diff touches only `rust-sak`'s entries and the new
      dependencies `github` pulls into `gui`, and `CARGO_HOME=$(mktemp -d) cargo fetch --locked` succeeds cold

## 2. The check and the link

- [x] 2.1 Add `crates/gui/src/update.rs` with the owner and repository constants and an async
      `#[tauri::command] is_outdated() -> bool` over `rust_sak::github::is_outdated_release(.., opai::version())`,
      mapping `Err` to `false` after a `tracing::warn!`; register it in `generate_handler![]`; verify unit tests pin the
      owner and repository, and that the error-to-`false` mapping logs (the network call itself is not unit-tested)
- [x] 2.2 Add `Link::Releases` to `links.rs`, crossing as `"releases"` and opening
      `https://github.com/vegidio/open-photo-ai/releases`, with the repository's address a macro both arms build on;
      verify the existing tests gain the new link's URL and name, and `"Releases"` is still refused

## 3. The frontend

- [x] 3.1 Add `isOutdated()` to `frontend/ipc/app.ts`, wrapped in `once()`, with its command name written once and a
      comment pointing at `update.rs`; widen `Link` in `frontend/ipc/links.ts` to include `"releases"`; verify
      `app.test.ts` pins the command name and that two calls make one `invoke`, and `links.test.ts` pins `"releases"`
- [x] 3.2 Render the "Update Available" button in `Navbar.tsx` after the version: shadcn `Button`, default variant,
      `size="sm"`, `ml-1 animate-pulse`, label `navbar.updateAvailable` (no new i18n keys, so no locale edits), shown
      only once `isOutdated()` resolves `true`, pressing it calling `openLink("releases")` and `console.error`ing a
      rejection; rewrite the version span's comment for the button now being present; verify `Navbar.test.tsx` asserts
      the button is absent before the answer, absent on `false`, absent on a rejection, present on `true`, and that
      pressing it sends `"releases"` and leaves the button shown when the open rejects

## 4. Checks

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test` for the workspace; verify all three are clean
- [x] 4.2 Run `pnpm typecheck`, the Vitest suite, and biome scoped to the files this change touched; verify all three
      are clean
- [x] 4.3 Hand the user a launch check on macOS: with the workspace at `26.9.0` the button appears, pulsing, after the
      version; pressing it opens the releases page in the default browser; with the network off at launch it does not
      appear and the log file carries the warning; verify the user confirms it
