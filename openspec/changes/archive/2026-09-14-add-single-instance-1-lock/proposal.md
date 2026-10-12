## Why

Three binaries are going to share one configuration directory, and two of them can already be
running at once. The reference implementation found this and said so, at `cmd/gui/main.go:159`:

> *"A second copy of the app is where the image cache's directory lock is genuinely contended: one
> directory, one lock, two processes. The library degrades to an in-memory cache rather than failing
> when it loses that race, **but the better outcome is not to race at all.**"*

It only ever enforced that for the GUI, because Wails handed it a single-instance option and the CLI
had none. This project is about to make the problem worse rather than better: a `perftest` binary
whose own documentation calls a full sweep *"an overnight job"* is precisely the process someone
leaves running while they open the GUI.

Three things are contended today, and the first is already silently broken:

- **The image cache.** `redb` takes an exclusive file lock on the directory — `rust-sak`'s `memo`
  README: *"a second open of a directory would normally fail — with an error indistinguishable from
  another process holding it."* `ImageCache::open` loses that race and says nothing, degrading `Disk`
  → `Memory` → `None`. `Opai::cache_mode()` reports the degradation and nothing obliges a caller to
  look at it, so "the second process silently ran uncached" is not observable from outside the
  machine.
- **Model and dependency installs.** The installer's contract is that every extracted file is
  recorded so replacing a version *deletes* the old files rather than merging into them. That
  contract assumes one writer of the manifest.
- **The log file.** The next change puts all three binaries in one `opai.log`. Daily rotation is a
  rename under a live writer, and a second process holding a descriptor keeps writing into the
  renamed inode.

Fixing them one at a time means three mechanisms guarding one invariant. There is one invariant: at
most one Open Photo AI process per user.

## What Changes

Task estimate: **15**. This is slice 1 of a two-slice single-instance series, and the first of six
changes in the plan design.md records.

**In `opai`, one lock and one new way for initialization to fail:**

- **An advisory file lock**, taken in `Opai::initialize` before it touches anything else and held for
  the life of the returned `Opai`. Advisory rather than a PID file for one decisive reason: the
  operating system releases the lock when the holder dies, including under `SIGKILL` or a power cut.
  A PID file survives a crash and leaves the application permanently unable to start, which is the
  classic form of this bug and the one worth designing out rather than mitigating.
- **Taken in `initialize`, not in `main`.** Everything that actually contends — the cache, the
  models, the runtime, the GPU — sits beyond that call. Everything that does not contend sits before
  it: `perftest list`, `--help`, `--version`, and today's whole `cli` binary, which prints
  `opai::version()` and exits. Locking at process start would refuse those for a reason that has
  nothing to do with them, and `perftest list` is documented as the command to run *first*, before
  anything is installed. Putting the lock where the shared state is also means the rule enforces
  itself in `opai` rather than in three `main` functions that each have to remember.
- **Per user, not per machine.** The lock file lives in the configuration directory, which is already
  per user. Two people signed into one machine have separate caches, separate models and separate
  logs, so they are not contending and are not refused.
- **The holder records who it is.** The application name and process id are written into a record
  beside the lock file, so a refusal can name what is holding it rather than saying only that
  something is. Beside rather than inside because Windows makes the locked file's bytes unreadable to
  exactly the process that needs them — design.md's D6 has it.
- **`App`** — `Gui`, `Cli`, `Perf` — the identity a binary declares when it initializes. It exists
  here because the holder record needs it, and the next change reuses it as the `app=` field on every
  log record. One three-valued identity, not two.
- **`InitError::AlreadyRunning`**, carrying the holder's application and process id, so a front end
  can render it and a terminal binary can exit non-zero having said something useful.

## Capabilities

### New Capabilities

- `single-instance`: at most one Open Photo AI process per user, and what happens to the second one.
  What is locked, when it is taken and released, what a refusal reports, and why a process that died
  without cleaning up does not lock the application out of itself.

  Distinct from the capability it constrains. `initialization` says what is put on disk and what has
  to be true before a model can run; this says how many processes may be doing that at once. It is
  also distinct from `image-cache`, whose degradation requirement stays exactly as written — this
  change does not alter what the cache does when it cannot open its store, it removes the most
  likely reason for that to happen.

### Modified Capabilities

- `initialization`: one requirement changes. Initialization currently has no failure mode that is
  about anything other than this machine's disk, network and hardware. It gains one that is about
  another process, and the ordering is part of the behaviour: the lock is acquired **before** the
  first probe, download or directory creation, so a refused second process leaves nothing behind and
  costs no transfer.

## Impact

**Crates:** `opai` only. No front-end code — `gui` and `cli` are still scaffolding, and `perf` does
not exist yet. Each will pass its `App` when it calls `initialize`; that is one argument at one call
site per binary, added by the changes that write those binaries.

**`rust-sak`:** nothing covers this. Checked the whole `fs` module — `mk_temp_*`, `user_config_dir`,
`mk_user_config_file`, the copy/move helpers, the three extractors — and there is no file lock, no
`flock` wrapper and no single-instance helper. Nor is there anything in `crypto`, `fetch` or `o11y`.

A cross-process single-instance lock **is** general-purpose, so by this project's own rule it is a
candidate for extending `rust-sak` rather than living here. That was raised and **deliberately
deferred**: the lock stays local to `opai` for now, to avoid blocking this series on a `rust-sak`
release cycle. design.md records it as a known debt with the conditions under which it should move.

**Dependencies:** none. The lock is `std::fs::File::try_lock`, stable since Rust 1.89 — the same
`flock`/`LockFileEx` pair the crates for this wrap. design.md's D2 records the revision and names the
alternatives it was chosen over.

**Behavioural divergence from the Go implementation**, and it is a widening rather than a gap: Go
locks the GUI only, through Wails' `SingleInstanceOptions`, and its CLI takes no lock at all. Here
the lock is in the library, so every binary that initializes takes it. Two consequences worth stating
plainly — running the Go CLI while the Go GUI is open is permitted and silently degrades the second
process's cache, and here it is refused; and Go's Linux path disables the lock entirely when no D-Bus
session bus is reachable, which a file lock does not need and does not do.

**Deferred to slice 2:** the GUI's *"raise the existing window instead of refusing"* behaviour. Go
does that and it is the right ending for a double-clicked icon, but there is no window to raise until
the GUI has one. This slice gives every binary the refusal; slice 2 turns it into a raise for the one
binary that can.
