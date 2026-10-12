## 1. The lock

- [x] 1.1 Take the lock with `std::fs::File::try_lock`, adding **no dependency**, and record D2 where
  the call is made — that std has carried this since Rust 1.89 over the same `flock`/`LockFileEx` the
  crates for it wrap, and that the workspace pins no `rust-version`. Verify `cargo build -p opai`.
- [x] 1.2 Add `App` (`Gui`, `Cli`, `Perf`) with its user-facing spelling, exported from the crate
  root. Verify a unit test round-trips every variant through that spelling and that adding a fourth
  variant would fail to compile the match.
- [x] 1.3 Add `InitError::AlreadyRunning`, carrying an optional holder `App` and process id, phrased
  so a terminal binary can print it unchanged. Verify a unit test asserts the rendered message both
  with a holder and without one.
- [x] 1.4 Write `opai::instance`: acquire an exclusive advisory lock on `<config dir>/instance.lock`,
  returning a guard that releases on drop, and refuse with `AlreadyRunning` when the lock is held.
  Verify a test acquires, observes a second attempt refused from a spawned child process, and
  observes it succeed once the first guard drops.
- [x] 1.5 Write the holder record — this process's `App` and pid, into `instance.holder` beside the
  lock rather than into the locked file itself, after the lock is taken — and the best-effort read of
  it for the refusal. Verify tests cover a well-formed record read back into the error *while the
  lock is held*, and that an absent, empty, truncated and unparseable record each still refuse while
  naming no holder (D6: contents never grant).

## 2. One lock per process

- [x] 2.1 Hold the guard in a process-wide registry keyed on the resolved configuration directory, so
  a second `initialize` under the same directory shares the first one's lock rather than contending
  with it (D5). Verify a test initializes twice in one process under one name and both succeed.
- [x] 2.2 Release the lock only when the last share is dropped. Verify a test drops the first
  initialization, asserts the lock is still held, drops the second, and asserts a fresh acquisition
  then succeeds.
- [x] 2.3 Verify two different application names in one process both acquire, each against its own
  configuration directory, by a test asserting both initializations succeed.

## 3. Wiring it into initialization

- [x] 3.1 Take the lock in `Opai::initialize` after the name is validated and the configuration
  directory resolves, and before the hardware probe, any transfer and any directory creation beneath
  it (D3). Verify a test asserts a refused initialization leaves the configuration directory
  byte-identical and performs no request against the test server.
- [x] 3.2 Hold the guard as a field on `Inner` so the lock's lifetime is the `Opai` handle's (D4).
  Verify a test clones an `Opai`, drops one clone, asserts the lock is still held, then drops the
  last and asserts it is released.
- [x] 3.3 Confirm a name that cannot be a directory segment still fails as a name rather than as a
  contended claim, since validation precedes the lock. Verify the existing invalid-name test still
  passes unchanged.

## 4. Proving the crash property

- [x] 4.1 Verify that a lock file left behind by a process that ran no cleanup does not lock the
  application out: a test that leaves a populated `instance.lock` on disk with no live holder, then
  asserts initialization succeeds. This is the property D1 chose the mechanism for, so it is checked
  rather than assumed.
- [x] 4.2 Verify the lock is released by an abruptly terminated holder, by a test that spawns a child
  holding the lock, terminates it without cleanup, and asserts the parent then acquires.
- [x] 4.3 Verify that what reads no shared state is not subject to the claim (D3's other half), by a
  test asserting `version()` and `catalogue()` both produce their result while a child process holds
  it. True today because neither resolves the configuration directory, which is a property of where
  they read from rather than of their signatures, so it is pinned rather than assumed.

## 5. Closing out

- [x] 5.1 Document the claim on `Opai::initialize` — that it is taken, when, that it is per user, and
  that it is released with the last handle — and note in `README.md` that only one OPAI process per
  user may run at a time.
- [x] 5.2 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, and
  verify all three are clean.
