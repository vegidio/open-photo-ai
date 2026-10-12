## Why

An analysis always reads every signal, so a caller that only cares about some families still pays for all of them. The
most expensive is the face signal, which installs, opens and runs the New York detector. A caller that wants to know
only whether a photograph should be upscaled and colorized has no way to say so, and no way to skip that work.

## What Changes

- `Opai::suggest` takes a new `families: Option<&[Family]>` parameter, between the picture and the options.
  - `None` behaves exactly as today: every family is checked.
  - `Some(families)` checks only the listed families. A family not in the list is never suggested, and the work that
    only it needs is not done: no face detection unless face recovery is asked for, no block pass unless denoise or
    sharpen is, and no strided pass unless light adjustment, colour balance or colorization is.
  - `Some(&[])` checks nothing and returns an empty, complete result.
  - `Family::Detection` in the list matches nothing, because it is never suggested.
- `Suggestions::incomplete` reports only signals that were asked for. A detector that would have failed but was never
  run leaves the result complete.
- The analysis log records which families were asked for, so that in a user's log a family that was never checked
  can be told apart from one that was checked and not suggested.
- **BREAKING** (public API): `Opai::suggest` gains a positional parameter. No caller outside `opai` exists today. The
  function-pointer pin in `tests/public_api.rs` and the doc example move with it.

Estimated at 6 tasks: one change, not a series.

### Deliberate divergences from the reference's behaviour

- **An analysis can be narrowed.** The reference's `SuggestEnhancements(ctx, input)` has no such parameter and always
  checks every family. With `None`, this library's behaviour is unchanged, so the only difference is that the option
  exists. It is worth adding because the face signal can install and run a model on a photograph nobody asked about,
  and a caller that doesn't want face recovery should not pay for that.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `autopilot`: an analysis can be restricted to a set of families. It suggests nothing outside that set, reads no
  signal that only an unrequested family needs, and reports incompleteness only for signals it was asked to read.

## Impact

- **Crates:** `opai` (`autopilot` and the `Opai::suggest` wrapper in `lib.rs`) only. Neither `gui` nor `cli` calls
  `suggest` yet, and nothing here is front-end logic.
- **Public API:** `Opai::suggest(&self, source: &Picture, families: Option<&[Family]>, options: Option<ExecuteOptions>)`.
  `Family` is already exported. `Suggestion` and `Suggestions` are unchanged.
- **`rust-sak`:** nothing it provides applies. This change only decides which of the existing signals run.
- **Dependencies:** none new.
