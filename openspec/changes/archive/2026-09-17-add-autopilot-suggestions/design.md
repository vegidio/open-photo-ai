## Context

See proposal.md — Why. The parts of the existing design that shape this one:

- `Opai::execute` already runs a single non-image operation, is served from the `Faces` store on a repeat, holds its
  own session handle for the length of the call and drops it before returning, and takes `ExecuteOptions` —
  provider, progress, cancellation and a per-run cache flag.
- `Operation::pipeline` is the one place that answers whether a family can be applied, refusing five of them with
  `UnsupportedReason::FamilyNotImplemented`. It is `pub(crate)` and generic in the backend, so a test inside the
  crate can ask it the question against the fake backend.
- `Family::ALL` is the declaration order of the eight families, and is the only canonical order of families the
  crate has. Its own documentation is explicit that it is *not* a presentation order, and the chain in
  `inference::process` applies operations in whatever order its caller supplies them — so there is no "order the
  enhancement path applies families" anywhere in the crate.
- `Scale` is bounded to 1.0–8.0 and quantized on acceptance, so 4x and 2x are constructible and no unbounded number
  reaches a suggestion.
- The reference's `autopilot.go`, its pixel scan, and the TypeScript `defaultUpscaleScale` in
  `cmd/gui/frontend/src/utils/enhancement.ts`.

This is **not one slice of a series**. It is a single change with two knowingly deferred parity gaps — the light
adjustment and colour balance signals — which are not slices of this work but consequences of those families not yet
having pipelines. Each is closed by the change that gives its family a pipeline, and that change writes the pixel
scan both share.

## Goals / Non-Goals

**Goals:**

- One entry point on `Opai` that answers "what does this photograph need?" for both front ends.
- A result that distinguishes *nothing needed* from *could not tell*.
- An analysis that costs at most one detection run, and ideally none, because the store already holds the answer.
- A shape that grows by an enum arm and a signal function per family, with no signature change.

**Non-Goals:**

- Turning a suggestion into an `Operation`. That needs the user's per-family model choice, which no settings store
  in `opai` holds yet. It belongs here when that store exists, not in a front end.
- Any policy about *when* to analyse — once per file, on selection, on import. That is the front end's.
- Reporting the execution provider the detection ran on. See the decision below.

## Decisions

### `Suggestion` carries the family and what the pixels imply, and nothing else

```rust
pub enum Suggestion {
    FaceRecovery,
    Upscale { scale: Scale },
}
```

**Why not `Vec<Family>`** (the literal Go port): the upscale scale is read off the same pixel count as the decision
to upscale at all, and Go splits those two halves across Go and TypeScript. A `Vec<Family>` here forces the CLI to
restate the TypeScript half, which is precisely the duplication the crate layering exists to prevent.

**Why not `Vec<Operation>`**: an `Operation` names a variant and a precision. Those are the user's standing
preference, and `opai` has nowhere to read one from — building them would mean either inventing a default the user
never chose, or inventing a settings store as a side effect of an autopilot change.

The arm for a family carries a parameter only where the *image* determines one. Face recovery's parameters —
which faces, at what fidelity — are not determined by "there are faces here", so its arm is a unit.

### The result is a struct, and a failed signal lives in it

```rust
pub struct Suggestions {
    pub suggested: Vec<Suggestion>,
    pub incomplete: Option<InferenceError>,
}
```

Three alternatives were weighed:

| | *nothing needed* vs *could not tell* | unprompted failure reaches the user | cost |
|---|---|---|---|
| bare `Vec<Suggestion>` (Go's shape) | indistinguishable | never | free |
| `Result<Vec<Suggestion>, _>` | distinguishable | as an error for something they did not ask for | free |
| `Suggestions` **(chosen)** | distinguishable | caller's choice | one struct |

This is the same reasoning that put `ProviderReport` on `Enhanced`: an outcome that is invisible in the result is an
outcome nobody can ask about. A portrait given no face-recovery suggestion because the machine is offline looks
exactly like a portrait the analysis considered and passed over.

`InferenceError` is not `Clone`, which is fine for a field and rules out handing the same failure to two places.

**Cancellation is not in this field.** `suggest` returns `Result<Suggestions, InferenceError>`, and
`Cancelled` and `Shutdown` are the error. A signal that could not be read leaves a question unanswered; a
cancellation withdraws the question. Concretely: the GUI cancels when the user selects a different file, and the
suggestions in flight describe the photograph they just left.

### The face signal goes through `Opai::execute`

`Analysis::Detection(Detection::newyork(FloatPrecision::Fp32))`, with the caller's options passed through.

The reference reaches past its own public API into `facerecovery.GetDtModel` and manages the lease by hand. Going
through `execute` instead buys three things it cannot have: the store serves a repeated analysis of the same
photograph without building a session; the analysis *warms* the store for the face-recovery operation the suggestion
turns into, so accepting a suggestion is free rather than a second detection; and the session lifetime is not this
module's problem.

FP32 is the reference's choice and its reasoning is transcribed into the spec: a suggestion is accepted at its
model's HD tier, so any other precision here installs a second detection graph to re-answer an answered question.

**Ownership and lifetime: this module owns no session and no lease.** `execute` acquires a handle, holds it for the
length of the call and drops it before returning — so `release_sessions` has nothing outstanding from an analysis,
and an analysis cannot outlive the session it ran on. That is the existing guarantee, and the decision here is to
depend on it rather than to reproduce the Go lease discipline.

### `ExecuteOptions` is reused rather than mirrored

All four of its fields mean something for an analysis: the provider to detect on, progress for the one run it makes,
the cancellation the decision above rests on, and the cache flag a benchmark needs to defeat.

`ProcessOptions` and `ExecuteOptions` are separate types because `depth` would be meaningless on a run that produces
no pixels, and "a field a caller can set that does nothing is a guarantee they can get wrong." That reasoning argues
*for* reuse here: a third near-identical type whose four fields all do exactly what the second type's do would be
duplication without a distinction.

### The provider report is not surfaced

`Executed<Faces>` carries a `ProviderReport` and `Suggestions` drops it. "Did this run get the GPU?" is a question
about a run the caller asked for; nobody asked for this one, and a caller that wants the answer runs the detection
itself through `execute` — which, after an analysis, is served from the store anyway.

### Order of evaluation, order of output

Evaluated cheapest first: area, then geometry, then the face signal. This is what lets the zero-area case return
before anything is installed.

Emitted in `Family::ALL` order — face recovery, then upscale — which is enforced by a test that sorts the produced
families against `Family::ALL` and compares, rather than by the order the pushes happen to be written in. A family
added later is then ordered by its position in the one list.

`Family::ALL` is chosen because it is the only canonical list the crate has, **not** because it is "the order the
enhancement path applies families" — there is no such order, per the Context above. That matters, because the
reference *does* have one list that is both presented and applied,
`['dn', 'fr', 'cl', 'la', 'cb', 'sh', 'up']` in `cmd/gui/frontend/src/utils/enhancement.ts`, and it disagrees with
`Family::ALL` about colorization, which it places before light adjustment and colour balance rather than after them.

Nothing can act on that today: colorization, light adjustment and colour balance all lack pipelines, so none of them
can be suggested, and over the two families that can be the two orders agree exactly. Rather than leave the
divergence in a comment, a test asserts *where* the two lists differ, so it fails if either moves. **The change that
gives colorization a pipeline owns the decision** — either reconcile the two orders, or make the present-and-apply
order something `opai` names in its own right (which is where it belongs under the crate layering, since both front
ends need it) and sort against that instead.

### "Nothing unrunnable is suggested" is held by a test, not by discipline

`Suggestion::family()` gives each arm its family. A test builds the family's operation and asserts
`Operation::pipeline` resolves it against the fake backend. Adding a `Suggestion` arm for a family whose pipeline is
still a refusal fails that test, which is what keeps the rule true as families land rather than relying on whoever
writes the next signal to remember it.

### Placement: `crates/opai/src/autopilot.rs`

Top level, beside `cache.rs` and `providers/`, declared in `lib.rs`. Not `models/`, which holds the vocabulary rather
than decisions taken with it. Not `inference/`, which holds only what no model owns — this owns a detection run.
`Opai::suggest` is the entry point, beside `process` and `execute`; the module holds the signals, the
scale ladder and their tests.

### `rust-sak`

Nothing needed and nothing missing. This change downloads, hashes, decodes and stores nothing of its own: the single
model run goes through `execute`, which already reaches `memo`, `fetch` and `crypto` through the paths built for it.
Nothing here is a candidate for extending `rust-sak` — a policy over this project's own families is general-purpose
to nothing. No new third-party crate.

### Crate split

`opai` only. `gui` and `cli` are untouched by this change. The GUI already persists the Autopilot toggle; wiring it
to a Tauri command, and mapping a `Suggestion` to an `Operation` against the user's model choices, is its own change
and is blocked on the settings store rather than on this.

## Risks / Trade-offs

**An analysis silently costs a model download on a fresh install.** → It is the detection model, which the
face-recovery path needs anyway, and `execute` reports the transfer through `on_progress`. The front end that wires
this up is responsible for showing that; the Go app shows a spinner per file for the same reason.

**`incomplete` is a field a caller can ignore.** → It can, and it then behaves exactly as the reference does today.
The field makes the better behaviour *possible*, which is all a library can do; the GUI change that consumes this
carries the task of using it.

**Two families is a thin autopilot.** → A user opening a dark, colour-cast photograph gets an upscale suggestion and
nothing about the exposure. That is the honest state of the application: suggesting light adjustment would produce an
operation the enhancement path refuses. The gap is recorded in the proposal so the change that adds those pipelines
closes it.

**The scale ladder is now in two places during the transition** — here and in the reference's TypeScript. → They are
not the same codebase and the Rust front end does not exist yet, so nothing can drift. The ladder is transcribed once,
with the thresholds pinned by boundary scenarios in the spec.

**Platforms and execution providers.** The signals themselves are platform-independent arithmetic and are tested
against the fake backend on any runner. The detection run inherits whatever `execute` was validated on and adds
nothing: `Auto` with CPU fallback, exercised on macOS in development. CUDA and TensorRT paths are untested by this
change and unchanged by it.
