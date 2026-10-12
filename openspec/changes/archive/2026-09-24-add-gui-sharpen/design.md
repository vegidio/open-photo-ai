## Context

See proposal.md - Why. The state this design builds on:

- **`opai` is finished for this family.** `VariantEntry::published(Family::Sharpen, codename)?.build(precision,
  &values)` resolves Moscow, Petersburg or Novgorod (`build.rs`) to `Sharpen::moscow`, `::stpetersburg` or `::novgorod`,
  and the catalogue publishes one parameter for the family, `strength`, as `Range { min: 0.0, max: 3.0 }` - the same
  `STRENGTH_PARAMETER` denoise publishes - with Moscow listed first. The codename is `petersburg`; only the library's
  constructor is spelled `stpetersburg`. What each strength does to a photograph, and Petersburg's divergence guard, are
  the `sharpen` capability's contract.
- **The wire has five arms.** `Requested` in `crates/gui/src/enhance/operation.rs` is tagged by family; its denoise and
  bias arms clamp and log. The test `a_family_this_application_cannot_run_is_refused_by_the_parse_and_names_it` sends a
  sharpen as its example of an unrunnable family. `ipc/enhance.ts`'s `Operation` union mirrors the arms, and
  `lib/enhancements.ts`'s `Unoffered` check fails the build if `RUNNABLE` omits a member of that union.
- **`IntensityOptions` already exists for this.** `add-gui-denoise` D2 built it as "a model and one amount", with
  `parameter`, `label` and `mark` as the only per-family choices, and named Sharpen as "one more wrapper".
- **Everything the frontend shows is already written.** `ENHANCEMENTS` carries a `sharpen` entry (name key, progress
  key, icon) between colour balance and upscale. Every locale carries `enhancements.sharpen.name`, the three model
  descriptions, `preview.progress.sharpen`, `enhancements.info` and `enhancements.strength`. The Settings dialog already
  stores a default sharpen model.
- **The test catalogue's sharpen entry has no parameters.** `test/support.tsx` elides parameters to `[]` for every
  family whose control is not built from them, so its three sharpen variants lack `STRENGTH`.
- **The reference** (`OptionsSharpen`) is `OptionsDenoise` with the prefix changed: `ModelSelector`, `Divider`,
  `IntensitySelector` over 0..300 with one mark at 100 and no label. `ENHANCEMENTS.sh` has `defaultAmount: 1` and
  `defaultModel: 'moscow'`.

## Goals / Non-Goals

**Goals:**

- Sharpen runnable end to end from the window, its panel indistinguishable from Denoise's but for the models.

**Non-Goals:**

- Folding the `Denoise` and `Sharpen` wire arms into one. `add-gui-color-balance` D1 keeps each family its own arm:
  serde needs one variant per tag, and the arm is where a reader looks for a family.
- Any change to `IntensityOptions`, `IntensityControl`, `opai`, sessions or execution providers.

## Decisions

### D1. The wire carries the library's unit strength, clamped and logged

`Requested::Sharpen { codename, precision, strength: f64 }`, resolved as
`ParameterValues::new().with_strength(Strength::clamped(strength))` with a `tracing::warn!` when `strength` falls
outside `Strength::MIN..=Strength::MAX`, then `VariantEntry::published(Family::Sharpen, ..).build(..)`. The denoise
arm's shape for the denoise arm's reasons (`add-gui-denoise` D1), written out as its own arm.

The two arms now share a clamp-and-warn block. It stays written out in each rather than pulled into a helper: the
bias arms already repeat the same shape for `Bias`, and a helper for two of the four would make the other two look
like the exception. If a third strength family ever arrives, that is the point to fold all of them.

### D2. The refused-family test moves to colorization

Sharpen gaining an arm breaks the test that uses it as the example of a refused family. The test keeps its purpose
and changes its example to `colorization` (`{ "family": "colorization", "codename": "delhi", ... }`), the one family
still without an arm. Its assertion - the refusal names the family - is unchanged. The doc comments in `operation.rs`
that count five families are updated to six.

### D3. `SharpenOptions`: one more wrapper

`features/enhancements/SharpenOptions.tsx` is `DenoiseOptions` with the family changed: `IntensityOptions` with
`parameter="strength"`, `label={t("enhancements.strength")}`, `mark={100}`, the amount read from and written to
`operation.strength`. Same props as every other panel (`enhancement`, `operation`, `entry`, `onChange`), so
`EnhancementRow`'s options switch gains one case.

**Not a shared `StrengthOptions` taking the family.** The two wrappers are about twenty lines each, and
`add-gui-denoise` D2 already decided the per-family file stays so a reader looking for "the sharpen panel" finds it.
Collapsing them would save one file and bring back the union-narrowing problem D2 avoided by writing through the
wrapper's own narrowed type.

### D4. The label is "Strength"

Decided by the user while exploring this change, overriding the design's screen and the reference, which both show
"Intensity" (the reference only because `OptionsSharpen` passes no label). Reasons: the parameter is `opai::Strength`;
`add-gui-color-balance` D2's rule (the label follows the parameter type) gives "Strength"; and Denoise, with the same
parameter, range and mark, already says "Strength". The existing `enhancements.strength` key is used; no key is added.

### D5. A new sharpen arrives at 1.0, with the catalogue's first model

`newOperation`'s `denoise` case becomes `case "denoise": case "sharpen":`, returning `strength: DEFAULT_STRENGTH` - the
fall-through the bias families already share, because the two produce the same shape. `DEFAULT_STRENGTH`'s doc is
widened to name both families; its value, 1, is the reference's `defaultAmount` for both.

The default model needs nothing: `modelChoice` falls back to the catalogue's first variant, which is Moscow - also the
reference's `defaultModel`.

### D6. Placed by the pipeline, not by the list

`sharpen` is added to `RUNNABLE` between `color_balance` and `upscale`. The menu draws `ENHANCEMENTS` filtered to
`RUNNABLE`, and `ENHANCEMENTS` already places sharpen there, so the position in `RUNNABLE` is documentation; its doc's
family count and order note are updated.

### D7. The row's info line

`EnhancementRow`'s info switch gains `sharpen` as a fall-through onto the `denoise` case - both read `strength`, so
unlike the bias/strength split in `add-gui-denoise` D6, the field is the same and a shared case is the honest shape.

### D8. The test catalogue gains sharpen's parameter

`test/support.tsx`'s three sharpen variants gain `STRENGTH`, and the fixture's doc, which lists the families whose
parameters it keeps, names sharpen. Without it the panel under test would find no range and the clamping tests would
be testing a fixture gap.

### D9. Where the work sits

| Crate | What |
|---|---|
| `opai` | Nothing. Everything this change calls is published and tested. |
| `gui` (Rust) | The `Sharpen` arm of `Requested`, its tests, the refused-family test's new example. |
| `gui` (frontend) | The union member, `RUNNABLE`, `newOperation`, `SharpenOptions`, the row's two switches, the test catalogue, tests. |
| `cli` | Nothing. The CLI already reaches sharpen through `opai`. |

`rust-sak`: no feature used, and no gap - the change contains no infrastructure. No crate or package is added.

### Sessions, providers and platforms

No session, memory budget or provider code changes. A sharpen is opened, cached and released by the same
`Opai::execute` path every other family is, under the three variants' existing `EpProfile`s (Moscow and Novgorod
restricted to CPU and GPU at FP16, Petersburg on defaults), and Petersburg's divergence guard runs inside `opai`. The
change is validated on macOS (CPU and CoreML through the existing profiles, via the GUI run). Windows and Linux are
untested here; nothing in the change is platform-specific.

## Slices

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-sharpen` | The `sharpen` wire arm; Sharpen in the add menu between Color Balance and Upscale at a strength of 100%; the `Moscow, 100%, HD` info line; `SharpenOptions` over the shared `IntensityOptions`, labelled Strength over 0..300% marked at 100 | - | **current** |

**This is a new series**, named after the feature. `add-gui-denoise` was a series of one and is closed.

## Risks / Trade-offs

- **[Moscow and Novgorod are slow Restormer models]** → a typed `150` still asks for three runs, each stopping the one
  before - inherited from `IntensityControl` and accepted in `add-gui-denoise`, where the same risk was recorded. A
  debounce would change every family's control and is not this change's decision.
- **[The label departs from the design's screen]** → one prop; `enhancements.intensity` exists in every locale if it is
  ever wanted instead. The decision and its reasons are recorded in D4 and the proposal.
- **[The fall-throughs in `newOperation` and the info switch make sharpen's behaviour implicit]** → each has a test
  naming sharpen (`newOperation("sharpen", ..)` carries Moscow at 1; the row reads `Moscow, 100%, HD`), so splitting
  the case later cannot silently change it.

## Migration Plan

Nothing to migrate. `Requested` gains an arm, which is additive on the wire; no stored preference changes shape, and
the default-model preference for sharpen already exists in the Settings dialog.
