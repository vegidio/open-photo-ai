## 1. The slot

- [x] 1.1 Add a named release to `Runs` that drops the held result when the slot is holding the given
      identity and does nothing otherwise, leaving it recorded in `produced` so it still answers as
      displaced (D1, D2), and verify tests cover releasing the held identity, releasing one the slot
      does not hold while a different result is held, and releasing while a run is in flight.
- [x] 1.2 Add the release-everything counterpart on the same terms, and verify a test covers it
      dropping a held result and leaving a run in flight to be stopped by its own path.
- [x] 1.3 Bound `produced` to a ring of the 64 most recent identities (D3), and verify tests cover an
      identity still answering displaced while it is in the ring, and answering unknown once it has
      fallen out.
- [x] 1.4 Mark a run the release names while it is still working, and have `finish` drop what a marked
      run hands back and report it as stopped (D5), and verify tests cover a run finishing after its
      own image was closed, after every image was closed, and a run over another image still keeping
      its result when a release names the one that closed.

## 2. The command

- [x] 2.1 Add the `release_enhanced` command beside `cancel_enhance`, naming the identity, answering
      nothing and failing at nothing, with the outcome in the log only, and register it in
      `src/lib.rs`'s handler.
- [x] 2.2 Add the release-everything command on the same terms, and verify tests cover both commands
      reaching the slot.

## 3. The frontend

- [x] 3.1 Add the two wrappers to `ipc/enhance.ts`, carrying the comment that pins each command name
      against its second spelling in `enhance/mod.rs`, and verify `ipc/enhance.test.ts` covers what
      goes on the wire for each.
- [x] 3.2 Call the release from the enhancement store's registered `FileOwner` - `forget` releasing the
      closed image's identity, `forgetAll` releasing everything (D4) - swallowing and logging a
      rejection rather than surfacing it, and verify tests cover a close releasing the right identity,
      a close of an image with no identity releasing nothing, and a rejection not reaching the user.

## 4. Verification

- [x] 4.1 Confirm a released result is served as `410 Gone` rather than `404`, end to end through the
      `opai://` scheme (D2), and verify a test covers it.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`; run
      `pnpm typecheck`, `pnpm test` and Biome over the touched frontend files.

No i18n keys: nothing here is user-visible.
