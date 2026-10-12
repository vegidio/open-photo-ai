## Context

`autopilot::suggest` has four steps: the zero-area guard, `geometry` (upscale), `pixels` and `face`. `pixels` does
two reads in one blocking task. The strided `pass` feeds light, colour cast and monochrome. The `blocks` read feeds
noise and sharpness, and sharpness also takes noise's luma estimate. `face` runs the New York detector through
`execute`. Once all four have run, the suggestions are sorted by `Family::ALL` and recorded. See proposal.md (Why) for
the motivation.

```
 family            signal        step that pays for it
 ---------------   -----------   ---------------------------------------
 Upscale           geometry      two integers                 (free)
 LightAdjustment   light    -+
 ColorBalance      cast      +-  pass::read   (~1M samples)
 Colorization      monochrome-+
 Denoise           noise    -+-  blocks::read (<=256 64x64 blocks)
 Sharpen           sharpness-+     (sharpness also reads noise.luma)
 FaceRecovery      face          model install + session + inference
```

## Goals / Non-Goals

**Goals:**

- Skip each of the three costly steps whenever none of the families it serves was asked for.
- Leave an analysis that is not narrowed exactly as it is now: same suggestions, same log, same errors.

**Non-Goals:**

- Skipping single measures inside a read that is already happening. Once the pass has been read, measuring light,
  cast and monochrome costs almost nothing next to reading it. Once the blocks have been read, sharpness needs the
  noise estimate anyway.
- Wiring narrowing into `gui` or `cli`. Neither calls `suggest` yet.
- Letting the caller control anything other than which families are checked, such as detection precision or the
  thresholds.

## Decisions

### 1. A positional `families: Option<&[Family]>` parameter

```rust
pub async fn suggest(
    &self,
    source: &Picture,
    families: Option<&[Family]>,
    options: Option<ExecuteOptions>,
) -> Result<Suggestions, InferenceError>
```

This was chosen over two alternatives:

- **A `SuggestOptions { families, execute }` struct.** It would make `suggest` stop mirroring `execute`'s
  `(source, …, options)` shape for the sake of one field. The doc comment on `suggest` names that mirroring as
  deliberate. There is no second autopilot-only option waiting to join `families`.
- **A `families` field on `ExecuteOptions`.** `execute` would carry a field it ignores.

A slice rather than `impl IntoIterator<Item = Family>`: `tests/public_api.rs` pins `suggest` as a plain function
pointer, and a generic parameter can't be pinned that way. A caller with a `Vec` or an array passes `&v` or `&[..]`.

`None` rather than an empty slice means "every family". An empty slice means "none". Making them mean the same thing
would give a caller who asked for nothing the most expensive analysis there is.

### 2. A `Copy` scope, built once

On entry, `suggest` turns `Option<&[Family]>` into a small `Copy` value, a membership set over `Family::ALL`, with
`includes(Family) -> bool` and helpers for each read: `needs_geometry()`, `needs_pass()`, `needs_blocks()` and
`needs_face()`, plus `needs_anything()`, which is true when any of the four is (decision 4). For the log (decision 5)
it also has `narrowed()`, whether the caller named a set at all, and `families()`, the included families in
`Family::ALL` order. It has to be `Copy` and owned because the pixel reads run in a `'static` blocking closure that
can't borrow the caller's slice. Duplicates and `Detection` fall out of the set semantics without a special case.

### 3. Each read runs only when needed; the suggestions are filtered once

- Before any read, `suggest` returns `Cancelled` if the analysis has already been cancelled. Until now the pixel
  step and the face step were the only places a cancellation was noticed, and a scope of `Upscale` alone reaches
  neither, so without this check a cancelled analysis would return an answer. For an analysis that is not narrowed,
  the pixel step starts with the same check, so its behaviour doesn't change.
- `geometry` runs only if `needs_geometry()`, that is, if the scope includes `Upscale`.
- `PixelSignals::read` takes the scope. It reads the pass only if `needs_pass()`, and otherwise leaves light, cast
  and monochrome as `None`. It reads the blocks only if `needs_blocks()`, and otherwise leaves noise and sharpness as
  `None`. The fields are already `Option`, and a `None` already means "concludes nothing", so no signal needs a new
  state. If neither read is needed, the blocking task is not spawned at all.
- `face` runs only if `needs_face()`. When it is skipped it adds nothing to `incomplete`.
- The final list is filtered by `scope.includes(suggestion.family())` **in one place**, just before the sort. This is
  what keeps a signal that was read for another family (light, when only colorization was asked for) from being
  suggested. It doesn't depend on each read knowing about each family.

Pruning each read alone, without the final filter, was rejected. It would need `suggestions()` to know about the scope
signal by signal, and the next signal added to the pass would silently leak into narrowed results.

### 4. A scope with nothing suggestible returns before any read

If the scope includes no suggestible family (an empty slice, or only `Detection`), `suggest` returns an empty,
complete `Suggestions` right after the zero-area guard and records it the same way. That is the same shape as the
zero-area early return. It keeps the spec's "no signal is read" true without depending on every read checking its own
flag.

### 5. The record names the scope

`record` gains one field, `asked`: `"all"` when the analysis was not narrowed, otherwise the labels of the requested
families in `Family::ALL` order. It is logged at `info` beside the existing `families`. For an analysis that is not
narrowed, the existing fields are unchanged.

## Division across crates

All of it is in `opai`: `autopilot/mod.rs` and the `Opai::suggest` wrapper and its doc comment in `lib.rs`. `gui` and
`cli` are untouched. No `rust-sak` feature is involved, and no crate is added.

## Platforms and execution providers

No session, memory budget or execution-provider logic changes. Skipping the face step means not calling `execute`,
and the call itself is unchanged when it is made. The tests run the stub backend and the pixel reads on macOS. They
depend on no platform or provider, so Windows and Linux are covered by the same tests in CI rather than by a separate
check.

## Risks / Trade-offs

- [A future signal is added to a read, and its family isn't added to that read's `needs_*` helper] → Its suggestion
  would never appear in a narrowed analysis that asks only for it. Mitigation: a test that goes through `Family::ALL`
  and, for each suggestible family, asserts that an analysis narrowed to it alone can still suggest it on a fixture
  that calls for it.
- [Breaking signature change] → No caller outside `opai`. The public API pin and the doc example are updated in the
  same change.
- [Callers may assume that an empty slice means "all"] → The doc comment states both meanings, and a test pins
  `Some(&[])` to an empty, complete result.
