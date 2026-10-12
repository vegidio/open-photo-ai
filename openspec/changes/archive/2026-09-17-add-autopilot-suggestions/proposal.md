## Why

Autopilot is the first thing the application does with a photograph: the GUI already ships the toggle, persisted and
on by default (`crates/gui/frontend/stores/enhancements.ts`), and `add-data-inference-cache` already named Autopilot
as a caller the `Faces` store was built for. What is missing is the thing the toggle turns on — nothing in `opai`
looks at an image and says what it needs, so every front end that wants suggestions would have to invent its own
answer.

The reference implementation answers it in `autopilot.go` with four heuristics. Two of them — light adjustment and
colour balance — are the *only* reason its single-pass pixel scan exists, and neither family has a pipeline here yet.
Suggesting an enhancement the library would refuse to run is worse than suggesting nothing, so this change delivers
the two signals whose families exist and does not write the scan at all. The scan arrives with the families that
need it, which is the change that can also test it against a real pipeline.

## What Changes

- **`Opai::suggest` analyses a picture and returns what it calls for.** Two signals today: a face signal that runs a
  detection and a geometry signal that reads the image's size.
- **A suggestion carries what the image implies, and names no model.** `Suggestion::FaceRecovery` and
  `Suggestion::Upscale { scale }`. The variant and precision to run are the user's standing preference, which this
  library does not hold, so the suggestion stops at the family and whatever the *pixels* determine.
- **The upscale scale ladder moves into `opai`.** In the reference it is split across two languages — Go's
  `shouldUpscale` decides *whether* at 4 MP and TypeScript's `defaultUpscaleScale` decides *how much* at 1 MP and
  4 MP — so the CLI would have had to restate it. It is one ladder and it lives here: ≤ 1 MP suggests 4x, ≤ 4 MP
  suggests 2x, larger suggests nothing.
- **Suggestions come back in `Family::ALL` order**, the one canonical family order the crate has — not "the order the
  pipeline applies them", which the crate does not have: the chain applies operations in the order its caller supplies
  them. Pinned as an ordering rule rather than a transcribed sequence, so a family added later lands in that order
  without anyone remembering to put it there, and the one place it diverges from the reference's present-and-apply
  list — colorization's position — is pinned by a test so the change that makes colorization suggestible has to
  settle it.
- **The face signal runs through `Opai::execute`, not around it.** The Go implementation reaches past its own public
  API into `facerecovery.GetDtModel` and holds a lease itself; here it is one `Analysis::Detection` at FP32, which
  means the run is served from the `Faces` store on a repeat *and* warms it for the face-recovery operation the
  suggestion turns into. The FP32 choice is the reference's own and its reasoning carries: a suggested enhancement is
  added at its model's HD tier, so detecting at any other precision downloads a second graph to re-answer an answered
  question.
- **A signal that could not be read is reported, not swallowed.** The reference logs a warning and returns `false`,
  which makes "there are no faces in this photograph" and "I could not look" the same empty answer. Here the result
  carries the failure beside the suggestions, so a front end can degrade quietly and still explain itself.
- **Cancellation fails the call.** A withdrawn question gets no answer, partial or otherwise — unlike a detection
  that was attempted and failed, which is one signal missing from an otherwise complete analysis.

**Not in scope:** the pixel scan and the light-adjustment and colour-balance heuristics, which belong to the changes
that give those families pipelines. No front-end work — wiring the GUI's existing Autopilot toggle to this, and
turning a `Suggestion` into an `Operation` against the user's model choices, is its own change and needs a settings
store that does not exist yet. No denoise, sharpen or colorization signal; the reference has none either.

**Task count: 12.** Within the 15-task limit, so this is one change rather than a series.

## Capabilities

### New Capabilities

- `autopilot`: what the application suggests for a photograph nobody has touched yet — which signals are read, what
  each one decides, the order suggestions come back in, what happens when a signal cannot be read, and what a run
  leaves in the log.

### Modified Capabilities

None. `data-inference` already requires everything the face signal depends on — a single non-image run, its result
type, its cancellation, and its being served from the store — and this change is a caller of that path rather than a
change to it.

## Impact

**Crates: `opai` only.** `gui` and `cli` are untouched. The suggestion-to-operation mapping the GUI will need is
deliberately *not* pushed into a front end: it needs the user's per-family model choice, and when that settings store
exists the mapping belongs in `opai` beside this.

**`rust-sak`: nothing needed, and nothing missing.** Checked before proposing anything else. This change downloads
nothing, hashes nothing, decodes nothing and stores nothing of its own — the one model run it makes goes through
`Opai::execute`, which already reaches `memo`, `fetch` and `crypto` through the paths built for it. There is no
candidate here for extending `rust-sak`: a policy over this project's own families is general-purpose to nothing.

**No new third-party crate.**

**Code:**

- `autopilot.rs` — new, at the top level beside `cache.rs` and `providers/`. It is not `models/`, which holds
  vocabulary rather than decisions, and not `inference/`, which holds only what no model owns; this owns a detection
  run and names two families, so it is its own module in the way `providers` is.
- `lib.rs` — `Opai::suggest` beside `process` and `execute`, the module declaration, and the re-exports.
- `tests/public_api.rs` — the new public surface pinned as the other entry points are.

**Deliberate divergence from the reference, in behaviour:**

1. **A failed detection is reported rather than logged and dropped.** Go returns a bare slice, so an autopilot that
   could not reach the network is indistinguishable from one that found a landscape. The parity gap is worth it
   because the user-visible failure it prevents — a portrait silently getting no face-recovery suggestion — is one
   the reference cannot even diagnose from its own return value.
2. **A cancelled run is an error rather than a partial answer.** Go's `shouldFaceRecovery` turns a cancelled context
   into `false` and carries on to return the geometry suggestion. Here the caller asked to stop, so nothing comes
   back.
3. **The scale ladder is in the library.** Behaviourally identical to the reference's split across Go and
   TypeScript, at the same thresholds; what changes is that a second front end cannot get it wrong.
4. **An image with no area is told it needs nothing.** Go's size test is `pixels <= 4 MP`, which zero passes, so its
   autopilot suggests upscaling an empty image. Its two scan-based heuristics each guard against an empty image
   explicitly, so this reads as an oversight rather than a decision, and it is not reproduced.
5. **Two families rather than four, today.** A knowingly open parity gap, closed by the changes that add the light
   adjustment and colour balance pipelines. Recorded here so it is picked up rather than forgotten.
