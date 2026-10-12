## Context

See proposal.md, "Why". The step being moved is `for_chain` in `crates/gui/src/faces.rs`. It is called from
`prepare_chain` in `crates/gui/src/enhance/run.rs`, and from there by both the canvas run and the export
(`crates/gui/src/export/run.rs`). It reaches the library through the GUI's `Enhancer` seam: `process`, plus a `detect`
whose default finds nobody, so test fakes that are not about faces need not think about them.

What `for_chain` decides today, which is what has to survive the move:

```text
  chain has no face recovery?  --> answer chain unchanged, NotAsked, detect nothing
  detect(Detection::for_face_recovery(), options)
     Ok(found)        --> each FaceRecovery := recovery.with_faces(found filtered by that op's choice)
                          answer Found(all faces, detector order)
     Err(Cancelled)   --> stop: answer no chain
     Err(other)       --> answer chain unchanged (recoveries keep their empty selection), Failed(sentence)
```

The window's per-face choice (`FaceChoice`: keys a user skipped or restored, falling back to `Face::restorable`) is
read per face-recovery operation, by zipping the requested list with the resolved one.

## Goals / Non-Goals

**Goals:**

- One implementation of the table above, in `opai`, that the GUI uses now and the CLI uses in slice 2.
- A selection the caller supplies per face recovery, plus a published default that is the GUI's "nobody chose"
  behaviour.
- No change to anything the GUI shows or does. The GUI's existing tests for chain faces still hold, moved to wherever
  the logic now lives.

**Non-Goals:**

- Any CLI code. That is slice 2.
- Changing `perf`'s `select::with_faces`. Its policy is intentionally different: every face found, and a row blocked
  rather than run when the detection fails, because a benchmark must not time a restore of nobody. It shares the
  detection preset already, and that is the only part that should be shared.
- Moving `FaceChoice`, `face_key`, `DetectedFace` or the picker's `detect_faces` command. They are the window's wire
  shapes and interaction, and only the window has a user choosing faces.
- Making `Opai::process` detect by itself. The `face-recovery` spec keeps "an operation finds none of its own", and an
  empty selection stays a valid request rather than "please detect".

## Decisions

### D1. Two public pieces: a pure fill, and the async step built on it

`opai` gains, in a new orchestration file `crates/opai/src/inference/faces.rs`:

- **`ChainFaces`**: `NotAsked | Found(Faces) | Failed(InferenceError)`. `InferenceError` is already `Clone`. The
  GUI turns it into its own sentence at the wire, as it does every other library error.
- **`FacedChain { operations: Vec<Operation>, faces: ChainFaces }`**.
- **A pure function** that takes the chain, the detection's outcome (`Result<Faces, InferenceError>`) and a selection,
  and applies the table above. It is `Err(Cancelled)` for a stop and `Ok(FacedChain)` otherwise.
- **`Opai::find_chain_faces(source, operations, select, options: Option<ExecuteOptions>)`**: returns `NotAsked`
  without detecting when no operation is a face recovery. Otherwise it runs `Opai::execute` with
  `Detection::for_face_recovery()` and passes the outcome to the pure function.

Why split it: the pure function holds every decision, so it can be tested exhaustively with no detector and no
backend. It is also what the GUI's test fakes use to answer the seam without restating the rules (D3). The async half
adds only "skip when not asked" and the one `execute` call. Internally it is written over a detect future, so that
half is also testable without a model on disk.

Alternatives considered:

- *Only the async method.* GUI fakes would then have to re-implement the table to answer the seam, which is the
  duplication this change exists to remove.
- *Make the step generic over a detector trait in the public API.* That exposes a testing seam as API surface for
  one consumer. The pure function gives the same testability with a plainer signature.

**Placement:** `inference/` is orchestration: running a data operation, feeding a chain. `Detection::for_face_recovery`
and `Operation::FaceRecovery` are `models` vocabulary, which `inference` may name. No model's code is named, and the
dependency still runs `inference -> models`.

### D2. The selection is a function of the recovery's position and the face

`select: impl Fn(usize, &Face) -> bool`, where `usize` is the operation's index in the chain as passed in, before
`Opai::process` puts it into apply order. That is exactly what the GUI needs, since it zips its requested list by
position. It also handles the theoretical chain with two face recoveries without a richer type.

The published default is `opai::default_face_selection(_: usize, face: &Face) -> bool`, which is `face.restorable()`.
That is a named function rather than "pass `Face::restorable`", so the spec's "default selection" is one item a
reader can find, and so the CLI and any later front end cite it rather than re-deriving it.

Alternative considered: a `FaceSelection` enum (`Default | Keys(...)`) in `opai`. Rejected: keys are the window's
encoding, and moving them into `opai` would pull the GUI's wire shape into the library.

### D3. The GUI's `Enhancer` seam trades `detect` for `find_chain_faces`

`Enhancer::detect` becomes `Enhancer::find_chain_faces(source, operations, select, options)`. The `Opai` impl calls
the library method. **The default implementation** calls the pure function with `Ok(Faces::empty())`, which preserves
the property every non-face fake relies on today: a chain without a recovery is `NotAsked`, and one with a recovery
restores nobody. Fakes that currently override `detect` (`FindingOne` in `enhance/run.rs`, `StoppedWhileDetecting`
in `export/run.rs`) override the new method by calling the same pure function with their own faces or
`Err(Cancelled)`.

`prepare_chain` passes `|index, face| requested[index].faces.clone().unwrap_or_default().keeps(face)` as the selection.
The window's choice is applied inside the library's step, but it is still defined in the window. `for_chain`, the
GUI's `ChainFaces`, and the `for_chain` tests in `faces.rs` are removed. The tests' assertions move to `opai` against
the pure function, except the ones about `FaceChoice` keys, which stay. The GUI converts `opai::ChainFaces` to its
wire fields where `enhance_with` already matches on it (`Found` maps through `DetectedFace::from`, and `Failed` uses
`to_string()`).

### D4. Sessions, providers and lifetimes are unchanged

The step makes no session of its own. The detection goes through `Opai::execute`, which already acquires the detector
through the session registry, holds it for the call, runs on the requested provider with CPU fallback, and serves a
repeat from the run store. The step holds nothing across calls, and the chain it answers is plain values
(`Operation` holding `Faces`, an `Arc<[Face]>`). The faces are in the coordinates of `source`. As today, the caller
must run the chain over that same picture, which `prepare_chain` does by construction because it frames first.

### D5. Crates and rust-sak

No crate is added. `rust-sak` is not used by this step and has nothing that covers it. It is application
orchestration, not infrastructure, so there is no gap to fill there.

## Risks / Trade-offs

- [The selection's index refers to the chain as passed, while `process` re-sorts into apply order] → The step never
  reorders and answers operations in input order, so indices stay meaningful to the caller. A test asserts that order
  is preserved.
- [GUI test fakes change shape across several files] → Mechanical. The default implementation covers most fakes, and
  only those that already overrode `detect` change.
- [A wire-visible regression in the GUI (faces found, failure notice)] → `enhance_with`'s existing tests for `Found`
  and `Failed` stay in the GUI and must pass unchanged.

## Validation

No model, session or provider behaviour changes, so there is nothing new to validate per provider. The GUI is
regression-checked on macOS (CoreML and CPU) with a face-recovery chain over `fixtures/test.dat`. Windows and Linux
are covered only by the test suite, with no manual run.

## Slices

The `add-cli` series. It delivers `opai <input> [--up [MODEL]] [--scale N] [--fr [MODEL]] [-q hd|sd] -o <output>`.

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-cli-1-face-detection` | Chain face finding moved from `gui` into `opai` with a published default selection. GUI switched to it with no behaviour change. | none | **current** |
| 2 | `add-cli-2-enhance` | The `cli` crate replaces its scaffold. Argument parsing: `--upscale/--up [kyoto\|osaka\|tokyo\|saitama]` (default tokyo), `--scale 1.0-8.0` (requires `--up`; when omitted, `suggested_scale` decides, and an image already large enough skips the upscale with a note), `--face-recovery/--fr [athens\|santorini]` (default athens), `--quality/-q hd\|sd` (default hd; hd = FP32, sd = FP16; for Osaka hd = FP16, sd = INT8), `--output/-o` required. Initialization with install and download progress, a clear message when the GUI holds the instance claim, load, `find_chain_faces` with the default selection, process (the library decides the order), save. | 1 | pending |

Decided for slice 2 during exploration and recorded here so they are not re-asked: flag order does not affect
apply order, and the input path comes first so a bare `--up`/`--fr` does not take it as a model name.
