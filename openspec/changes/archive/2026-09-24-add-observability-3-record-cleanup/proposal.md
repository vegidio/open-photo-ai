## Why

Since slice 2, every record `opai.log` gets at `info` and above also reaches Grafana. So the records now answer
questions across the whole installed base, not only for one user who sends their file. An audit of the records `opai`
writes found five kinds of fault, each of which makes those answers wrong:

- **Some failures leave no record at all.** A failed `initialize`, a session that would not build on the CPU, an
  image that would not decode or encode, and an analysis refused before it starts all produce nothing in `opai`.
- **Some failures are recorded twice.** A failed step writes "step failed" and then "enhancement failed" with the same
  error. A refused chain writes "chain refused" and then "enhancement failed". An unreadable face signal writes
  "analysis failed" and then "a signal could not be read". `gui` then records most of these a third time, at `error`
  where `opai` said `warn`.
- **A cancellation is recorded as a failure.** A user dragging a slider cancels runs, and each one writes a warning
  that says "enhancement failed".
- **Some closing records cannot be read on their own.** "initialized" and "the dependency is ready" say nothing about
  how long the work took. The closing records of enhancements and analyses do not name the photograph.
- **Two levels break the crate's own rule.** Two records use `error` for things the application continues past.

Slice 4 puts spans on these same units of work, so the records need to be right before that.

## What Changes

The whole feature needs well over 15 tasks, so it is split into seven slices (see design.md - Slices). This proposal
is the third, at **13** tasks.

**In `opai`: failures that are silent today are recorded**
- **Initialization is recorded from start to finish.** "initialized" gains `duration`. A failure is recorded once with
  its duration and error. A refusal because another process holds the claim, and a runtime shutdown, are recorded as
  what they are, not as failures. "ONNX Runtime started" gains the duration of the library load.
- **A session that would not build says so** from inside the single build, once however many requests were waiting on
  it, with its duration. The record for the fallback to the CPU no longer repeats the error, which the build record
  now carries.
- **Image IO records its own failures.** `load`, `save`, `encode`, `probe`, `probe_raw` and `identity` each record a
  failure once, at `warn`, naming the operation, the file and the error. Success stays silent, because slice 4 gives
  image IO spans.
- **A refused analysis says so**, as a refused chain already does. Both are judged before their run opens, so a
  refusal writes exactly one record and no start/finish pair.

**In `opai`: each failure is recorded once**
- **The failed step is folded into the run's own record.** "step failed" is removed, and "enhancement failed" gains
  the failing step's `operation` and `index`.
- **The chain is planned before "enhancement started"**, so a refusal writes "chain refused" and nothing else.
- **"a signal could not be read" is removed.** Autopilot's conclusion record gains `incomplete` and `error`, and the
  face detection's own "analysis failed" record still holds the failure.

**In `opai`: cancellations are split from failures**
- **A run that was cancelled or abandoned ends in a "stopped" record at `info`**, with its reason (`cancelled` or
  `shutdown`) and duration. This covers enhancements, analyses, autopilot, initialization and dependency installs, and
  none of them says "failed" any more. A session build ended by a shutdown is recorded the same way.

**In `opai`: fields and levels**
- **Every closing record can be read on its own.** Enhancement and analysis closing records carry `identity`. Their
  opening records carry the requested `provider`. Dependency installs record their `duration`. The autopilot
  conclusion records its `duration`.
- **`error` is reserved for what the process does not survive.** The refusal to install an unpublished model and the
  run cache that could not be opened become `warn`. After this slice, `opai` writes `error` only for a panic.

**In `gui`: records that repeat `opai`'s are removed**
- **15 records are removed.** Each one repeats a failure `opai` now records itself: initialization,
  enhancement, detection, analysis, export chain, export write, the image reads behind them, preview decode/encode,
  and file describe. The code around each stays: the error still goes back to the window, it is just not logged a
  second time. The one thing lost is `gui`'s `run` id beside the failure, which slice 5 restores for free: its
  command span carries `run`, and the file writes a span's fields onto every record inside it.
- **Failures only `gui` can see keep their records unchanged.** Examples are claiming or committing an export's
  destination, and a reveal naming an unknown file.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `logging`:
  - A unit of work that was stopped is recorded as stopped, not failed.
  - Initialization is accounted for from beginning to end.
  - A failure is recorded once, by the layer where it happened. A front end does not record it again at all.
  - Reading and writing an image records its failure.
  - Severity follows what the application does next.
  - Every closing record names the unit's subject and duration.
  - A refused analysis is recorded, and a failed enhancement names its step in its own record.
  - Building a model that fails is recorded.
  - An installation says how long it took.

`autopilot`'s requirement that a record name the unread signal and its reason is still met, now by the conclusion
record. Its spec does not change.

## Impact

**Crates touched:**
- `opai`: `lib.rs`, `runtime.rs`, `setup.rs`, `deps/install.rs`, `deps/model/mod.rs`, `sessions/`,
  `inference/process/`, `inference/execute.rs`, `autopilot/`, `image/`, and the tests beside each.
- `gui`: the log levels of the records listed above, and nothing else. No logic moves into a front end: this slice
  takes records out of `gui`, because `opai` is where the failures are known.
- `cli` and `perf`: no change. They gain the new `opai` records through the same subscriber.

**`rust-sak`:** nothing is needed. `o11y` and its bridge, as released for slice 2, carry whatever records `opai`
writes. No pin moves.

**Dependencies:** none added, none changed.

**Parity divergence from the Go application:** none that a user can see. This slice changes only what reaches
`opai.log` and the collector. The records are this project's own, and the series already diverges from the Go
application, which exported error-level logs only.

**Not in scope:**
- Spans and metrics on units of work (slice 4).
- `gui`'s command wrapper, the Analytics switch, and the other `gui` records that are not repeats of `opai`'s
  (slice 5).
- The frontend (slices 6 and 7).
