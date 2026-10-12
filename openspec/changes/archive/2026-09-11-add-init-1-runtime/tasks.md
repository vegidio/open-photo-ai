## 1. `rust-sak` - the in-stream digest (a separate repository, `../rust-sak`)

- [x] 1.1 Add `DigestAlgorithm`, `RequestOptions::digest` and `Download::digest` to `fetch`, hashing bytes as they are written and hashing the existing `.part` prefix before appending on a resume; verify a fresh transfer's digest equals `crypto::sha256_file` of the result and that a download not asking for one still reports `None`
- [x] 1.2 Cover the digest in `fetch`'s tests against the local test server - fresh transfer, resumed transfer, and a transfer restarted because the sidecar did not match - and verify all three produce the digest of the whole artifact
- [x] 1.3 Document the option in `src/fetch/README.md`, run `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test`, then **get the owner's go-ahead**, push, bump the manifest to the next CalVer and cut the tag; verify the tag resolves with `gh api repos/vegidio/rust-sak/tags`

## 2. `opai` - foundations

- [x] 2.1 Re-pin `rust-sak` to the new tag in `opai/Cargo.toml`, add its `crypto` feature, add `tokio` (`rt`, `fs`, `sync`), `serde`, `serde_json` and `thiserror`, and commit `Cargo.lock`; verify a cold `cargo build --locked -p opai` succeeds
- [x] 2.2 Add `error.rs` (`InitError` via `thiserror`, with named variants for an unsupported platform, a hash mismatch and an invalid application name) and `config.rs` resolving the config directory over `fs::mk_user_config_dir`; verify tests cover a rejected name (empty, `.`, `..`, containing a separator) and a resolved `runtime` subdirectory under a temporary config root
- [x] 2.3 Add `deps/artifact.rs` and `deps/release.rs` - the pinned table copied verbatim from the Go `internal/artifacts.go` (`runtime/1.26.0`, five platforms, hash + size + library name), the `std::env::consts` to asset-name mapping (`macos`→`darwin`, `x86_64`→`amd64`, `aarch64`→`arm64`), the release URL, and the dependency descriptor built from a resolved artifact; verify tests assert the URL for each pinned platform and that an unmapped platform resolves to an error rather than an empty hash

## 3. `opai` - the install pipeline

- [x] 3.1 Add `progress.rs`: public `Progress`, `Phase` and `Dependency` (both enums `#[non_exhaustive]`), and the reporter composing the download and expansion phases; verify tests cover the 0.8/0.2 phase split, the monotonic clamp, and a final report of exactly `1.0`
- [x] 3.2 Add `deps/manifest.rs`: the record, `fingerprint` over the tag and each source URL and expected hash, atomic write, and `intact`/`remove`; verify tests cover a missing, malformed and wrong-schema record all reading as absent, `intact` using `symlink_metadata` so a symlink chain is not read as corruption, and `remove` pruning emptied subdirectories while keeping the install directory
- [x] 3.3 Add `deps/install.rs`: fingerprint check and early return, clearing the previous install (manifest `remove`, or emptying an unrecorded exclusive directory while keeping `.part`/`.part.json`), download with `resume_key` set to the pinned hash, verify the digest (falling back to `crypto::sha256_file` when the transfer short-circuited on an existing file), expand on `tokio::task::spawn_blocking` with `fs::extract`'s `on_progress` hook wired into the reporter, record the tree, delete the archive, write the manifest last; verify with the tests in 4.1, and that extracting the fixture with a callback observes reports arriving before the expansion finishes
- [x] 3.4 Implement the single retry on a hash mismatch - delete the bytes, re-run once with `DownloadMode::Overwrite`, fail with both hashes if the second attempt also mismatches; verify tests cover a mismatch that recovers on the second pass and one that does not
- [x] 3.5 Add `Opai::initialize(name, on_progress)` in `lib.rs`, installing the runtime dependency into `<name>/runtime/` and returning the `Arc`-backed handle; verify a test drives a full install against the local server and asserts the extracted files, the written manifest and the final progress report

## 4. Verification

- [x] 4.1 Commit a small 7z fixture archive (as `internal/deps/testdata/tree.7z` is in Go) and a local `tokio::net::TcpListener` test server serving it with `Range`, `ETag` and mismatched-hash responses, then cover the pipeline end to end: first install, second launch installing nothing, a recorded file deleted forcing a reinstall, a bumped pin removing the previous version's files, an interrupted install (record removed, files present) reinstalling, and an interrupted download resuming rather than restarting
- [x] 4.2 Widen `.github/workflows/ci.yml`'s test job to a three-OS matrix (ubuntu, macos, windows), replacing the comment that defers it, and keep fmt and clippy on one runner; verify all three jobs pass on the branch
- [x] 4.3 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked` in this workspace, and confirm the same three are green in `../rust-sak`
