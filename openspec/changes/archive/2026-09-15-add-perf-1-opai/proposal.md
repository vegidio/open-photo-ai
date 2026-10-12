## Why

A benchmark is the next thing this project builds, and three facts it depends on are currently
locked inside `opai`. `perftest` has to drop every resident session between models — otherwise its
"cold start" quietly reuses a session the warm-up built, and the number that is supposed to describe
session construction describes a map lookup. It has to know whether the provider it asked for is the
provider that ran, because a silent GPU→CPU downgrade is otherwise indistinguishable from a slow
GPU, which is the single most misleading thing a benchmark can report. And it has to be able to
measure a model that has been re-exported but not yet published, which today is impossible: the
installer sees a hash that does not match and downloads the published weights back over the file
under test.

None of this is benchmark-only. A front end wiring a shutdown or a "free memory" gesture needs the
first, and a front end that wants to tell the user *"your GPU could not open this model, so it ran on
the CPU"* needs the second — which is why all three belong in `opai` rather than in the harness that
prompted them. This slice unlocks the harness without writing any of it.

## What Changes

- **`Opai::release_sessions` becomes public.** The method, its behaviour and its record already
  exist; it has been `pub(crate)` with a `dead_code` allowance since the sessions slice, which said
  in writing that the first caller would be *"the front end that wires shutdown"*. That caller has
  arrived. No behaviour changes — the allowance and the comment predicting this come off.

- **BREAKING: `Opai::process` returns what the run ran on, not only the picture.** It returns a
  bundle carrying the enhanced `Picture` beside a report of the execution provider that was
  requested and the provider(s) sessions were actually built on. The fact already exists on
  `SessionHandle` — `requested()` beside `provider()` — and is discarded at the end of the chain.
  Breaking in the strict sense only: `gui` and `cli` are still scaffolding and neither calls
  `process`, so no call site moves.

- **`Opai::initialize` takes the trust it places in the model files on disk**, as a field of
  `InitOptions`. It declares, once for the process, whether a model already present is used as it is
  or checked against its published hash. Only a debugging or benchmarking binary sets it; `gui` and
  `cli` take the default.

  *Not* breaking, and that changed after this was written. `refactor-init-options` landed
  `InitOptions` immediately before this slice, saying in writing that the bundle exists so *"a field
  added later is source-compatible for every caller — which is the whole reason the options are a
  bundle rather than more parameters, and is what this change exists to buy before the next one adds
  one."* This is that next one, so the field goes in the bundle and `initialize`'s signature does
  not move. Nothing the declaration is required to be is affected: it is still made at
  initialization and only there, there is still no setter and no per-run field, and a process that
  did not ask for it when it started still cannot arrive at it later.

- **A model already on disk can be used unverified, under four conditions carried from the
  reference.** It applies only when *every* file backing that model is already present; the identity
  recorded is stamped from the files as they sit on disk, so a re-export invalidates what an
  execution provider compiled from the old weights and an unchanged file does not; the record names
  no files, so a later run with the flag off reinstalls from the published sources rather than
  trusting what was never verified; and a warning is recorded each time it is taken.

- **Deliberate divergence: a download is always verified.** The reference also ignores a hash
  mismatch on bytes it just fetched from the network (`internal/deps/download.go`, `acquire`'s
  `skipVerify` branch). This project carries the on-disk half and not that one — see below.

- Nothing is added to the model install path that a front end can reach. Skip-verify is not a
  `ProcessOptions` field and not a setter; a process that did not ask for it at initialization
  cannot arrive at it later.

## Capabilities

### New Capabilities

None. Every behaviour here extends a capability that already has a spec.

### Modified Capabilities

- `image-enhancement`: a run reports the execution provider it was asked for beside the one(s) it
  actually ran on, and the result of a run is that report together with the image rather than the
  image alone.
- `model-sessions`: releasing every resident session is something a consumer of the library can
  actually do, rather than something the library merely contains.
- `model-install`: a process may declare that models already on disk are used as they are; what that
  covers, what it does not, and what it records.

## Impact

**Crates:** `opai` only. Nothing is added to `gui` or `cli`: neither calls `initialize` or `process`
yet, and the option and the return type arrive before their first call site does. The consumer
that prompted all three is `crates/perf`, which is slice 6 and is not written here.

**rust-sak:** nothing new is needed, and nothing here duplicates a module. `crypto` already backs
every hash the installer takes, `fetch` already owns the transfer, and the on-disk stamp this adds
is a name, size and mtime per file — `std::fs::symlink_metadata`, the same call the manifest's
`intact` check already makes on every launch — rather than a hash of gigabytes of weights, so it
reaches for no hashing at all. No `rust-sak` gap, and so nothing to extend there.

**Public API:** `Opai::process` changes signature; `Opai::initialize` gains an `InitOptions` field
rather than an argument (see above); `Opai::release_sessions` appears. Two new exported types (the
run's result bundle and its provider report) and one new exported enum (the model-trust
declaration). `crates/opai/tests/public_api.rs` pins the signatures through function pointers and is
updated with them.

**Code:** `lib.rs` (the return type, the `InitOptions` field and the visibility), `inference/chain.rs` (the report
composed and returned), `sessions/mod.rs` (`clear`'s `dead_code` allowance), `deps/release.rs` (the
trust carried on a `Dependency`), `deps/model/descriptor.rs` (where a model's descriptor takes it)
and `deps/install.rs` (the unverified branch).

**Behavioural divergence from the Go implementation, and why it is worth it:** `SetSkipModelVerification`
there also suppresses the hash check on a file downloaded during the same run. This project spent an
earlier change removing the last path on which an unverified artifact could be installed — a missing
platform row answered with a zero value, and *"an empty expected hash means do not verify"*, which is
how the NVIDIA libraries once shipped unverified (`deps/artifact.rs`). Reintroducing it through a
debug flag would undo that for the one case it was written for: bytes arriving over the network.
The use this exists for is a file the developer put there by hand, which is fully served by the
on-disk half. So skip-verify here decides whether local files are trusted; it never decides whether
downloaded ones are.

**Estimated tasks:** 14 — within the 15 the project allows, so this stays one change. The series it
belongs to is listed in `design.md`; this is slice 5 of 7 and slices 6 (`add-perf-2-harness`) and 7
(`add-perf-3-tui`) are proposed when they are reached.
