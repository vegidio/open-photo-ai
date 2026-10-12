## Why

Colorization is the last family `Operation::pipeline` refuses. Slices 1 and 2 built and proved every piece of
arithmetic a run needs: the Lab round trip, the stretch, both graph contracts' inputs and chroma readers, and the fused
compose at the run's depth. None of it is reachable, because nothing runs a graph between those pieces. This slice
writes that pipeline, gives the three models their measured profiles, and removes the refusal. With the refusal gone,
every enhancement family this project publishes runs.

Removing the refusal also removes the last producer of `UnsupportedReason::FamilyNotImplemented`. The variant describes
work in progress, and once that work is done it describes nothing, so it is deleted rather than left public with no
producer.

This is slice 3 of a three-slice series, and the first one with observable behaviour.

## What Changes

**The pipeline** (`models/colorization/process.rs`, family tier). One graph run per photograph, whatever its size:

- A photograph with no area is refused before anything is allocated.
- The variant's contract builds the input tensor: stretch to the square, then gray-Lab or ITU-601 luma.
- The graph runs once.
- The contract reads the chroma: the a/b planes directly, or extracted from RGB.
- `compose::composed` puts the chroma onto the photograph's own L at the depth the caller asked for.
- Progress is reported over three counted steps, and cancellation is checked at each step boundary.

**The two contracts as data on the variant.** A two-arm `Graph::{Ab, Rgb}` is returned by an exhaustive
`ColorizationVariant::graph()`: Delhi and Mumbai `Ab`, Jaipur `Rgb`. This is the decision slice 1 recorded. The pipeline
reads its square, output channel count, input builder and chroma reader from the arm, and from nothing else.

**The three models' directories and profiles.** `delhi/` and `mumbai/` are new, and `jaipur/` gains its profile. Each
declares CoreML restricted to the CPU and GPU at FP16 only, and the provider defaults at FP32. These settings are
ported from `delhi.go`, `mumbai.go` and `jaipur.go` and **not re-measured**. Mumbai's and Jaipur's files carry the
reference's reasoning. Delhi's says that the reference records no measurement of its own for it.
`ColorizationVariant::profile()` becomes a match that reaches each model's file.

**The refusal removed.** `Colorization::pipeline` is added. `Operation::pipeline`'s `Colorization` arm hands over to it,
and every arm of that match is now a hand-over. The `expect(dead_code)` attributes slices 1 and 2 added all come off.

**BREAKING (library API): `UnsupportedReason::FamilyNotImplemented` is deleted.** The enum is `#[non_exhaustive]`, so a
caller that matched it already has a catch-all arm. What remains is `IncompleteGraphSet`, which no shipped variant
produces. What else goes with it:

- `tests/public_api.rs`'s deferral pin is removed, and the `Unsupported` error's rendering is pinned through
  `IncompleteGraphSet` instead.
- The refused-family tests in `models/operation.rs` are deleted.
- The four `inference/process.rs` tests that used a refused operation to test what a refusal does are deleted, with
  their `unservable()` fixture. This was decided with the user on 2026-09-23.
- `perf`'s refusal test moves to `IncompleteGraphSet`. The fixtures in `perf`'s `main.rs`, `json.rs`, `report.rs` and
  `viewport.rs` that show Delhi as declined move off colorization.
- The prose that names colorization as refused is updated: `Opai::process`'s "What is refused", `Operation::pipeline`,
  and the doc comment on autopilot's runnable-families test.

**The tests.** A fake-backend suite runs on every CI platform. It checks, for each contract:

- the graph's shapes;
- which input each contract builds;
- that the photograph's own lightness survives at full resolution;
- that the colour comes from the model;
- the depth, transparency, progress, cancellation and zero-area refusal.

An `#[ignore]`d live check runs all three models at both precisions on the CPU provider and on CoreML.

### Deliberate divergence from the reference implementation

The run keeps the photograph's depth, and the resampler, flattening and rounding differ from the reference. Each of
these was recorded by slice 1 or slice 2 and is unchanged here. This slice adds one:

- **Progress is reported at three counted steps** (the input built, the graph run, the result composed), not at the
  reference's 0 and 1. The graph is a fixed cost, and the compose is proportional to the photograph. On a large
  photograph, a bar that moved only at the start and the end would sit still for the compose. This is light
  adjustment's reasoning, applied again.

Cancellation is checked where the reference checks its context: before the stretch, after the input is built, and
after the graph run. That is parity, not a divergence.

### Infrastructure: what `rust-sak` already covers

**Nothing applies.** This is a pipeline over code the crate already has: the session seam, `SingleGraph`, `checkpoint`
and slices 1 and 2's arithmetic. `rust-sak` has no inference or colour module that bears on it. **No new third-party
crate.**

### Task estimate

**12** against the 15-task budget:

- two for the graph dispatch and the three profiles;
- five for the pipeline, its wiring and its fake-backend suite;
- three for deleting `FamilyNotImplemented` and everything that pinned it;
- two for the live check and the toolchain pass.

## Capabilities

### New Capabilities

- `colorization`: what a colorization run does to a photograph. The whole photograph is shown to the model once,
  stretched to the model's square. The result keeps the photograph's own lightness and resolution, and only the colour
  comes from the model. The run keeps the photograph's depth, and transparency is composited away. All three variants
  run. The spec also covers progress, cancellation, and the refusal of a photograph with no area.

### Modified Capabilities

- `execution-providers`: adds Delhi's, Mumbai's and Jaipur's measured settings, as every family before this one added
  its own.
- `image-enhancement`: "A run is refused rather than served wrongly where no pipeline exists" is replaced by "…where an
  operation cannot be served". The replacement drops the "cannot be run yet" answer, the two scenarios that describe it,
  and the rule against a third answer that distinguishes families from variants, because nothing can produce that
  answer once every family runs. It adds a scenario that every published enhancement is served. The refusal itself, the
  rule against falling back to the unchanged image, and refusal before anything installs all stay. The change is a
  REMOVED plus ADDED pair because OpenSpec cannot drop scenarios from a MODIFIED requirement.

## Impact

**Crates touched: `opai` and `perf`.** `perf` changes only in tests and fixtures that named the deleted variant or used
Delhi as their example of a declined model. `gui` and `cli` are untouched:

- `cli` runs no operations yet.
- The GUI's IPC wire (`gui/src/enhance/operation.rs` and `RUNNABLE` in `frontend/lib/enhancements.ts`) carries only
  face recovery and upscale. A family reaches the GUI by gaining an arm there, in a change of its own. Light
  adjustment, colour balance, denoise and sharpen run in the library today and are not on that wire either.
  Colorization joins them. It is not a gap this slice opens.

After this slice, colorization runs through `Opai::process` and `perf`.

| File | Change |
| --- | --- |
| `crates/opai/src/models/colorization/process.rs` | new: `Graph`, the pipeline, and the fake-backend suite |
| `crates/opai/src/models/colorization/{delhi,mumbai}/mod.rs` | new: each model's profile and its prose |
| `crates/opai/src/models/colorization/jaipur/mod.rs` | gains Jaipur's profile and its prose |
| `crates/opai/src/models/colorization/variant.rs` | `profile()` a match over the models, and `graph()` |
| `crates/opai/src/models/colorization/mod.rs` | `pipeline()`; the `expect(dead_code)` attributes removed; header no longer says refused |
| `crates/opai/src/models/colorization/live.rs` | new: the `#[ignore]`d live check |
| `crates/opai/src/models/operation.rs` | the refusal arm becomes a hand-over; refused-family tests deleted |
| `crates/opai/src/error.rs` | `FamilyNotImplemented` deleted |
| `crates/opai/src/inference/process.rs` | four refusal tests and `unservable()` deleted |
| `crates/opai/src/lib.rs`, `autopilot.rs` | prose naming colorization as refused |
| `crates/opai/tests/public_api.rs` | the deferral pin removed, and the rendering pinned through `IncompleteGraphSet` |
| `crates/perf/src/{sweep,main,json,report,viewport}.rs` | the refusal test and the declined-model fixtures |

**Public API:** one variant is removed from a `#[non_exhaustive]` enum. Nothing is added.

**Downloads.** The six graphs are 0.44 to 0.91 GB each, 4.0 GB in total. A `perf` sweep that names no models now runs
all three colorization models, because it runs whatever the application can run. The live check keeps its weights in
`live_support::kept_dir`, as Osaka's does, so that a second run does not download them again.

**Risk.** Anything wrong in the wiring still renders a plausible photograph:

- a swapped contract;
- the output read at the wrong channel count;
- the compose handed the wrong square.

Each of these is a separate fake-backend assertion. The arithmetic underneath is slice 1's and slice 2's, and is
already pinned bit for bit.
