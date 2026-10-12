## Slices

Six changes, in order. Single-instance is the first because the two after it are designed against
its guarantee: the logging change puts three binaries in one file and needs to know whether a second
writer is possible, and the benchmark needs to know that nothing else is competing for the GPU, the
cache or the model directory while it is measuring. **This change is single-instance slice 1.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **current** |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | pending |
| 3 | `add-logging-1-file` | `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, the `---` session divider, the session header, the per-record `app=` field | 1 | pending |
| 4 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | pending |
| 5 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 4 | pending |
| 6 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 5 | pending |

Slice 2 is separated from this one by a dependency this project does not have yet rather than by
size. `gui` is still scaffolding; there is no window to raise. This slice gives every binary the
refusal, and slice 2 turns the refusal into a raise for the one binary that can do better.

## Context

See proposal.md — Why for the motivation, and specs/single-instance/spec.md for the behaviour.

Four facts about the existing code shape this design and were read rather than assumed:

- **`Opai` is `Clone` over an `Arc<Inner>`** (`lib.rs:305-312`), *"so a GUI can hand one to
  `app.manage()` and a CLI can keep one in a local without either being a second initialization."* A
  lock held as a field on `Inner` therefore has exactly the lifetime wanted: released when the last
  clone drops, and never earlier.
- **Two `Opai`s in one process is a supported state, not an error.** `Opai::initialize`'s
  documentation: *"Calling this more than once in one process is supported and is what a front end
  restarting its application layer after an error does."* This is the design's sharpest constraint —
  see D5.
- **The image cache already solves this exact problem for its own lock.** `Inner::cache`'s comment:
  *"Opening the same directory twice in one process yields the one store rather than failing on its
  lock, so two `Opai`s under one name share what either of them stored."* `rust-sak`'s `memo` does
  that by handing back the store the process already has open. D5 is the same answer in the same
  shape.
- **`initialize` resolves the configuration directory as its first real act**, through
  `config::app_dir(name)`, which validates the name and creates the directory. The lock needs a
  directory to live in, so it cannot be strictly first — D3 says what "first" actually means here.

## Goals / Non-Goals

**Goals:**

- One invariant, enforced in one place: at most one process per user is past `Opai::initialize`.
- A process that dies without cleaning up — crash, `SIGKILL`, power loss — does not lock the
  application out of itself on the next launch. This is the property the design is chosen for, not
  one it mitigates afterwards.
- A refusal names what is holding the lock, so the user is told which application to close rather
  than that "something" is running.
- Commands that contend for nothing keep working while another process holds the lock:
  `perftest list`, `--help`, `--version`, and today's whole `cli` binary.
- Two `Opai`s in one process go on working exactly as they do today.

**Non-Goals:**

- Raising the GUI's existing window. Slice 2, and it needs a window.
- Coordinating two processes so they *can* run together — a shared-cache protocol, a lock server, a
  broker. The invariant is that they do not, and this is the cheap way to hold it.
- Guarding anything below process granularity. Sessions, the cache and the installer each have their
  own in-process synchronisation and keep it.
- Protecting against a deliberately hostile local process. An advisory lock coordinates cooperating
  participants; it is not a security boundary, and every participant here is this application.

## Decisions

### D1. An advisory file lock, not a PID file

The lock is an exclusive advisory lock on a file in the configuration directory — `flock` on Unix,
`LockFileEx` on Windows.

The decisive property is what happens when the holder dies badly. **The kernel owns the lock, not the
file's contents**, so it is released when the last descriptor on it closes — which the operating
system does for every descriptor of a dying process, whatever killed it. There is no cleanup path to
get wrong and no state to go stale.

A PID file is the obvious alternative and is wrong here. It survives the process that wrote it, so
every launch has to decide whether the recorded pid is a live OPAI or a pid the kernel has since
recycled onto something else. Getting that wrong in one direction permanently bricks the
application; in the other it silently permits the second process the mechanism exists to refuse.
That reasoning is not hypothetical for this application: the thing the lock most needs to survive is
a **native crash inside an inference session**, which is exactly the failure the reference's own
issue #40 records.

A lock-file-plus-liveness-probe hybrid is the same trade with extra steps. A named semaphore or a
D-Bus name — what Wails uses — means three mechanisms for three platforms, and the reference's
`hasSessionBus` escape hatch exists precisely because one of them is not always reachable. A file
lock needs one mechanism and no escape hatch.

### D2. The standard library takes the lock, and nothing is added to take it

**`std::fs::File::try_lock`**, stable since **Rust 1.89** along with `lock`, `lock_shared`, `unlock`
and `std::fs::TryLockError`. It is `flock(LOCK_EX | LOCK_NB)` on Unix and
`LockFileEx(LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)` on Windows — the same two system
calls, with the same advisory semantics, and **no dependency at all**.

This decision was **revised during implementation**. It originally chose `fs4`, and the revision was
forced by a compile error rather than by a preference: `file.try_lock()` resolved to the inherent
`std::fs::File` method rather than to `fs4`'s trait method, and the two `TryLockError` types would
not unify. `fs4`'s own source records why — its error type *"mirrors `std::fs::TryLockError` that was
stabilized alongside `File::try_lock` in Rust 1.89."*

The rejected alternatives, and they are the same three as before with one more now above them:

- **`fs4` 1.1.0** — pure Rust over `rustix` and `windows-sys`, the maintained successor to `fs2`, and
  what this design first chose. It wraps the calls std now exposes, and additionally offers async
  lock variants, `fs-err` integration, `allocated_size`/`allocate`, filesystem stats and a 1.75 MSRV.
  This change uses **none** of those: one blocking `try_lock` on a `std::fs::File`.
- **`fd-lock`** — the same advisory semantics again, last released over a year ago, with an API built
  around a guard that *borrows* the file, which is a poor fit for a lock stored in a struct.
- **`fs2`** — unmaintained, no release in eight years.

The original argument for adding a crate at all was that writing `rustix` and `windows-sys` by hand
meant hand-testing the Windows path to save one manifest line. std removes exactly that cost: the
cross-platform path is written and tested by the standard library.

**MSRV is not a constraint here.** The workspace pins no `rust-version`, deliberately — *"an
application makes no MSRV commitment"* — so nothing is being held below 1.89.

### D3. The lock is taken in `Opai::initialize`, immediately after the configuration directory resolves

Not in `main`, and not before the directory exists.

Everything the lock protects is beyond `initialize`: the image cache, the model and dependency
installs, the loaded runtime and the GPU. Everything that contends for none of it is before —
`perftest list` is documented as the first thing to run and does no initialization at all,
`--version` and `--help` touch nothing, and today's entire `cli` binary is `println!("{}",
opai::version())`. A lock in `main` would refuse all of those while another process is running, for a
reason that has nothing to do with any of them.

Putting it in the library also means the invariant cannot be forgotten. Three binaries calling
`initialize` get the lock whether or not their authors thought about it; three `main` functions each
remembering to take one is three chances to not.

**Ordering inside `initialize` is part of the behaviour**, not an implementation detail, and the spec
says so: the name is validated and the configuration directory resolved — the lock needs somewhere to
live — and then the lock is acquired **before** the first hardware probe, the first transfer and the
first directory creation beneath it. A refused second process must leave nothing behind and must not
have spent a byte of network.

### D4. The lock's lifetime is the `Opai` handle's

The guard is a field on `Inner`. `Opai` is `Clone` over `Arc<Inner>`, so the lock is released when
the last clone is dropped, and a GUI that hands a clone to `app.manage()` while keeping one in a local
does not release it by dropping either.

No explicit release call, and deliberately none: an `unlock()` method would be a way to keep an
`Opai` whose invariant no longer holds. Dropping the application is how you stop being the
application.

### D5. Two `Opai`s in one process share one lock

The constraint from Context: a second `initialize` in the same process is supported and must keep
working. A naive second `flock` of the same path from the same process does not fail cleanly — on
Unix a lock is held per open file description, so a second open takes a second, independent lock
attempt and blocks or fails against the process's own first one.

So the lock is held in a **process-wide registry keyed on the resolved configuration directory**. The
first `initialize` under a directory opens and locks the file and records the guard; a later one under
the same directory observes the record and takes a share of it. The lock file is closed, and the lock
released, when the last share is dropped.

This is the same answer `rust-sak`'s `memo` already gives for the same problem one layer down —
*"`Memo::disk` and `Memo::memory_disk` therefore hand back the store this process already has open
rather than trying to open it again"* — and the same answer `Inner::cache`'s comment describes. A
third mechanism with different semantics for the same situation would be the thing to avoid.

Two `Opai`s under **different** names are two configuration directories and two lock files, and both
succeed. That is correct and matches how `sessions` and `cache` already reason about it: they are two
applications that share nothing.

### D6. The holder record is best-effort, never authoritative, and in a file of its own

The application name and process id are written after the lock is taken, and read by a process that
fails to take it, so the refusal can say *"already running (gui, pid 4821)"*.

**Beside the lock file, not inside it** — `instance.holder` next to `instance.lock`. This was
**revised during implementation**, and the reason is Windows. std takes the lock with
`LockFileEx(LOCKFILE_EXCLUSIVE_LOCK, .., u32::MAX, u32::MAX)`, which locks the whole file and makes
that region unreadable and unwritable through every *other* handle — including a second handle in
the locking process's own. A record kept inside the lock file is therefore unwritable by the holder
and unreadable by the process that was refused, so on Windows every refusal would name no holder
while the tests for it went on passing on Unix, where `flock` guards the open file description and
blocks no I/O whatsoever. The lock file is now opened, locked and never read from or written to.

Two files cost nothing, because the contents were never load-bearing in either arrangement: what the
kernel guards and what a message is built from are different things, and this makes them different
files. It also removes the one write this design ever aimed at a file another process might hold.

**The record's contents decide nothing.** The kernel's answer to the lock request is the only thing
that decides whether this process may proceed. The contents are a courtesy for the error message, and
every failure to read them — absent, empty, truncated, half-written, from a version that wrote a
different format — degrades to a refusal that names no holder rather than to an error or, far worse,
to permission to proceed.

Keeping those two apart is the whole point. The moment the record's contents can grant or deny
access, every failure mode of a PID file is back, and D1 was chosen to be rid of them. A write of it
that fails takes the previous holder's record with it, so the one thing it cannot do is leave a stale
name to be read as the live one.

### D7. `App` is defined here and reused by the logging change

`App::Gui`, `App::Cli`, `App::Perf` — what a binary declares when it initializes.

It is in this slice because the holder record needs it: the refusal is much more useful for naming
which application to close. The next change needs the identical three-valued identity as the `app=`
field on every log record, so that two sessions interleaved in one `opai.log` can be told apart. One
type, defined once, rather than a lock-holder string and a logging enum that can disagree.

Closed rather than open — a `&str` would let a fourth value in without anyone deciding there should
be one, and there are exactly three binaries.

### D8. Per user, and that follows from where the file lives

The lock file is in the configuration directory, which each platform already resolves per user. Two
people signed into one machine get two directories, two caches, two model stores and two logs, and
nothing they do contends. Refusing the second of them would be refusing a user their own application
because someone else is using theirs.

Nothing enforces this beyond the file's location, and nothing needs to: the scope of the lock is the
scope of the state it protects, which is the property worth having.

### D9. The lock stays in `opai` rather than extending `rust-sak`, and that is a recorded debt

**`rust-sak` covers none of this.** The whole `fs` module was read: `file_exists`, `mk_temp_dir`,
`mk_temp_dir_in`, `mk_temp_file`, `mk_temp_file_in`, `user_config_dir`, `mk_user_config_dir`,
`mk_user_config_file`, `list_path`, `copy_files`, `move_files`, `extract`/`extract_as`/`unzip`/
`un7zip`/`untar_xz`, and the `TempHandle` and `SafePath` types. There is no file lock, no `flock`
wrapper and no single-instance helper, and nothing in `crypto`, `fetch`, `memo`, `o11y` or `sysinfo`
either.

By this project's own rule that is a reason to extend `rust-sak`: a cross-process single-instance
lock is general-purpose and another application could want it. **It is deliberately not being done
now**, at the user's direction, so that this series is not sequenced behind a `rust-sak` release.

Recorded as debt rather than settled, with the conditions that should move it: a second first-party
project wanting the same lock, or this one growing a second kind of cross-process coordination. The
module is written so that move stays cheap — it depends on `std`, a path and this crate's own `App`,
so it is liftable very nearly as-is. D2 making it dependency-free is what leaves nothing at all to
reconcile with `rust-sak`'s own manifest on the day it moves.

`rust-sak` features this design otherwise uses: **none new.** The configuration directory it puts the
lock file in is already resolved through `fs::mk_user_config_dir` by `config::app_dir`.

### D10. Files named for what they are, beside the state they guard

`<config dir>/instance.lock` and `<config dir>/instance.holder`, siblings of `cache/`, `models/`,
`runtime/`, `libs/` and the `logs/` the next change adds. The first is locked and never otherwise
touched; the second carries the holder record — D6 says why they are two files rather than one.

Not inside `cache/`, though the cache is the most contended thing: the lock also guards the model
directory and the log file, and a lock living inside one of the three things it protects invites
someone to delete it with that directory. Not in the system temp directory, which is neither
per-user on every platform nor guaranteed to survive.

A lock file of its own rather than locking one that already exists for another purpose. The
alternative worth naming is leaning on the cache's `redb` lock, which is already exclusive and
already cross-process — but `memo` reports its failure as an error *"indistinguishable from another
process holding it"*, so a corrupt database or a read-only disk would refuse to start the
application. The lock has to be able to say *"someone else has it"* and nothing else.

## Risks / Trade-offs

**A network filesystem gives advisory locks unreliable semantics** → `flock` over NFS and SMB is
famously either emulated, silently a no-op, or subtly wrong. The configuration directory is a local
per-user path on all three platforms, so this needs a deliberately unusual setup — a roaming Windows
profile on a share, a network-mounted `$HOME` — to reach. Not defended against: the failure is that
two processes both start, which is where the application already is today.

**The refusal is a worse experience than Go's raise-the-window** → For the GUI, and only until slice
2. A user who double-clicks the icon while it is already running gets an error instead of their
window. Accepted deliberately for one slice, because the alternative is holding the whole series
behind a GUI that has no window yet.

**A user running two OPAI binaries on purpose is now refused** → This is the point, but it is a real
loss: no benchmarking while the GUI is open, no scripted CLI run alongside a GUI session. The
mitigation is D3 — everything that does not contend stays available — and the justification is that
the permitted version of this silently degraded the second process's cache without telling anyone.

**A process wedged rather than dead holds the lock forever** → A hung OPAI keeps its descriptor open,
so the kernel keeps its lock, and the next launch is refused with the holder's pid. That is the
correct answer and the pid in the message is what makes it actionable. There is no override flag, and
adding one would mean shipping a documented way to produce the corruption the lock exists to prevent.

**The lock rests on a recently stabilised standard-library API** → `File::try_lock` landed in Rust
1.89 and this design leans on it entirely rather than on a crate that has wrapped the same calls for
longer. The exposure is a toolchain floor, not a behavioural one: the system calls underneath are
`flock` and `LockFileEx`, which is what every candidate crate would have issued. Bounded by D9 — the
module is liftable into `rust-sak` without touching its callers, and swapping the two lines that call
std for a crate would be the same shape of change.

## Platforms

The design targets **macOS, Windows and Linux**, and the standard library provides the same advisory
semantics on all three.

**Validated on none of them yet** — this is a design, and the honest statement is that the only
platform this project's live checks have ever actually run on is macOS arm64
(`inference/live.rs`: *"The NVIDIA providers remain unverified"*). The test tasks cover the
behaviour that can be checked hermetically on every CI platform: acquire, refuse, release on drop,
release on an abandoned lock file, two `Opai`s in one process, and a holder record that is absent or
malformed. What CI cannot check is a real second process on a real Windows share, and the risks
above say so rather than implying coverage that does not exist.

D6's split of the holder record out of the lock file is the one place where "the same semantics on
all three" turned out to be false in a way a macOS run could not show: the lock behaves identically
everywhere, and the I/O *around* it does not. The tests that read a record beside a held lock are
what make the Windows leg of the matrix a check rather than a formality.

The Windows leg caught this on its first run, and caught it in the tests rather than in the
implementation: two of them asserted that the lock file is never written to *by reading it*, which
`flock` permits and `LockFileEx` refuses with `ERROR_LOCK_VIOLATION`. The production path never
touches that file and needed no change. Both tests now compare its length instead of its bytes — all
the lock leaves observable, and all a change to an always-empty file could be.

**Execution providers are untouched.** The lock is taken before any provider is resolved and has no
interaction with session construction, memory budgets or fallback.

## Division across the crates

**All of it in `opai`.** The lock guards state that `opai` owns — the configuration directory, the
cache, the model store — and the rule the project sets is that anything two front ends would need
lives there. Three front ends need this one.

**`gui`, `cli` and `perf` contribute one argument each**, the `App` they pass to `initialize`, added
by the changes that write those binaries. No front end holds, releases, checks or reasons about the
lock; the only thing any of them does with it is render `InitError::AlreadyRunning` in its own idiom
— a dialog, a line on stderr, a non-zero exit.
