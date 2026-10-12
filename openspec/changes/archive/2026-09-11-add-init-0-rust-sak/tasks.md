## 1. `rust-sak`: partial-file bookkeeping in `fetch`

Tasks 1.1-1.3 happen in `../rust-sak`, a separate repository. Nothing is pushed there until task 4.1
has the owner's go-ahead; work on a branch until then.

- [x] 1.1 Implement the `.part` file and its sidecar per design D1: transfers write to `<path>.part`
      with a `<path>.part.json` recording `url`, `etag`, `total` and `resume_key`; on success the body
      is flushed and synced, the sidecar removed, and `.part` renamed onto the final path. Implement
      D1's resume state machine in full - a sidecar whose `url` or `resume_key` differs, and a `.part`
      with no sidecar at all, both delete and start fresh rather than appending. `DownloadMode::Overwrite`
      ignores any existing `.part`; `DownloadMode::Skip` tests the final path only, which is now
      sound because that path is only ever complete. Verify `cargo build` succeeds and that the
      existing `fetch` tests still pass unchanged except where they assert on the old on-disk layout.

- [x] 1.2 Add `RequestOptions::resume_key(impl Into<String>)` and send `If-Range` on a resume where an
      `ETag` was recorded, per design D2. The `200` answer a changed resource produces is already
      handled as a fresh download - confirm that path is reached rather than adding a second one.
      Verify the builder round-trips the key and that a recorded etag reaches the request headers.

- [x] 1.3 Test the bookkeeping against a local HTTP server, covering each row of design D1's table:
      a completed transfer leaves no `.part` and no sidecar; an interrupted one resumes from the
      recorded length; a **changed URL writing to the same local filename discards the partial instead
      of appending to it** (the bug this slice exists for); a `.part` with no sidecar restarts; a
      mismatched `resume_key` restarts; `If-Range` answered with `200` restarts; and a cancelled
      transfer leaves nothing at the final path, so a following `DownloadMode::Skip` does **not** skip.
      Verify each test fails if its guard is removed.

## 2. `rust-sak`: extraction progress in `fs`

- [x] 2.1 Add `ExtractProgress` and `ExtractOptions::on_progress` per design D3, wired into the entry
      copy loop so it reports **within** a large entry and not only between entries, and read
      `total_entries`/`total_bytes` from the ZIP central directory and the 7z header, reporting `None`
      for TAR.XZ. Per design D4, do not touch name validation, the cap-std anchored handle, symlink
      re-resolution, mode masking, deferred directory modes or the reserve-before-write accounting.
      Verify `ExtractOptions` still derives/implements what it did before, `cargo build` succeeds, and
      the existing extraction tests pass unchanged.

- [x] 2.2 Test the hook: a multi-entry ZIP reports non-decreasing `bytes` ending at the summary's
      `bytes` and `entries` at its `files`; a **single-entry archive larger than one copy chunk
      reports more than once**, which is the ONNX Runtime case design D3 names; ZIP and 7z report
      `Some` totals matching the summary; TAR.XZ reports `None`; and an extraction with no hook set
      behaves exactly as before. Verify each test fails if the hook call site is removed.

## 3. `rust-sak`: versioning and documentation

- [x] 3.1 Set the manifest version to CalVer per design D5 (`26.9.0`, or the month the tag is actually
      cut - see design Open Questions), and update `src/fetch/README.md` and `src/fs/README.md`: the
      `DownloadMode` table's `Resume` and `Skip` rows, the new `.part`/sidecar on-disk layout, the
      `resume_key` and `If-Range` behaviour, `ExtractOptions::on_progress`, the `ExtractProgress`
      fields, and the contract that the callback must not block. Note the TAR.XZ `None` explicitly so
      it does not read as a bug. Verify both READMEs describe the behaviour the tests in 1.3 and 2.2
      assert, with no stale claim that a resume appends to the final path.

- [x] 3.2 Run the full `rust-sak` gate: `cargo fmt --all --check`, `cargo clippy --all-targets --locked
      -- -D warnings` and `cargo test --locked`, with the feature combinations its CI uses rather than
      only the default set - both new APIs sit behind `fetch` and `fs`, which have no default
      enablement. Verify all three exit 0 and `git status --porcelain` is empty afterwards.

## 4. Release and adoption

- [x] 4.1 **Get the owner's explicit go-ahead, then push and tag.** `rust-sak` is a published
      first-party crate shared with other projects and this slice carries a behavioural break (design
      D1, and Risks). Present what changed, what breaks, and the version decision from D5 before
      pushing anything. Then push the branch, merge it, and cut the tag matching 3.1's manifest
      version. Verify `git ls-remote --tags` shows the new tag and that the tagged manifest version
      equals the tag name - the disagreement design D5 exists to end.

- [x] 4.2 Add the dependency to `opai/Cargo.toml` per design D6 and D7: `rust-sak = { git =
      "https://github.com/vegidio/rust-sak", tag = "<4.1's tag>", features = ["fetch", "fs"] }`, with no
      `version` key. Commit the updated `Cargo.lock`. Verify a **cold** resolve - `rm -rf ~/.cargo/git/checkouts/rust-sak-*`
      then `cargo build -p opai --locked` - succeeds, `cargo tree -p opai` shows the crate at the
      tagged revision, and `Cargo.lock` records that exact revision rather than a branch.

- [x] 4.3 Run this repository's full gate and confirm CI resolves a git dependency: `cargo fmt --all
      --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, plus the
      frontend scripts from `gui/` if anything there moved (it should not). Verify all three exit 0,
      both lock files are tracked by git, `git status --porcelain` is empty after a build, and the CI
      run on the branch is green - the git dependency needs network at build time, which no previous
      change in this repository exercised.

- [x] 4.4 Update `README.md`: `opai` now has a dependency, and it is a git dependency pinned to a tag
      rather than a registry crate. Say why in one line (`rust-sak` is first-party and unpublished) so
      the next person does not read it as an oversight, and note that bumping it means editing the tag
      in `opai/Cargo.toml` and refreshing `Cargo.lock`. Verify by reading it back - someone cloning
      this repository should understand the pin without opening this change's artifacts.
