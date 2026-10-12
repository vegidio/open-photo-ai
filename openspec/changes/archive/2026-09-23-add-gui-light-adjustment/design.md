## Context

See proposal.md - Why. The state this design builds on:

- **`opai` is finished for this family.** `VariantEntry::published(Family::LightAdjustment, codename)?.build(precision,
  &values)` resolves Paris or Lyon from the catalogue, `ParameterValues::with_bias` carries the amount, and
  `Bias::clamped` is documented as *"what a GUI slider wants"*. The catalogue publishes one parameter for the family,
  `bias`, as `Range { min: -1.0, max: 1.0 }`, per variant.
- **The wire has two arms.** `Requested` in `crates/gui/src/enhance/operation.rs` is tagged by family, and a family with
  no arm is refused by serde at parse time. `ipc/enhance.ts`'s `Operation` union mirrors it, and
  `lib/enhancements.ts`'s `Unoffered` type check fails the build if `RUNNABLE` omits a member of that union.
- **Two options panels exist**, both assembled the same way inside `EnhancementRow`'s `OptionsPanel`: `ModelTray`,
  a `Separator`, then the family's own control (`ScaleControl`, or the faces block). `UpscaleOptions` reads its bounds
  from the selected variant's published `scale` parameter by name.
- **`components/ui/slider.tsx`** already has `track={false}` for a slider about a meaningful zero (the crop dialog's
  fine rotation), and `formatValue` for a bubble. Radix's `onValueCommit` fires on pointer release and on keyboard
  adjustment.
- **Every locale already carries** `enhancements.info` (`{{name}}, {{intensity}}%, {{quality}}`) and
  `preview.progress.lightAdjustment`, ported whole in `add-gui-1-foundation`. There are thirteen locale files.

## Goals / Non-Goals

**Goals:**

- Light adjustment runnable end to end from the window, looking as screen 11b draws it.
- An intensity control that the three later intensity families can be handed without being rewritten.

**Non-Goals:**

- Opening a panel when its enhancement is added (screen 11b's note is wrong; see proposal.md).
- Denoise, sharpen or colour balance on the wire or in the menu. The control takes a range, but no second caller
  is written for it.
- Any change to `opai`, to sessions or to execution providers.

## Decisions

### D1. The wire carries the library's unit bias, clamped and logged

`Requested::LightAdjustment { codename, precision, bias: f64 }`, resolved as
`ParameterValues::new().with_bias(Bias::clamped(bias))`, with a `tracing::warn!` when `bias` falls outside
`Bias::MIN..=Bias::MAX` - the `Upscale` arm's shape exactly.

- **Unit value rather than a percentage.** The percentage is how this window presents the number; the library, the
  catalogue's range and the run cache all speak in `-1..1`. Converting on the Rust side would put a presentation rule
  in the one place the wire is meant to be the library's own vocabulary, and would make the catalogue's published
  range and the wire's accepted range different numbers.
- **Clamped rather than refused**, for the reason the scale requirement already records: the control is bounded, so an
  out-of-range value is a fault, and a picture plus a log line is a better answer to a fault than a failed run.
  `Bias::clamped` also maps `NaN` to 0, the neutral bias.

*Alternative considered:* `bias: Bias` directly, deserialised through its `try_from = "f64"`. Rejected: that refuses
out of range at parse time, which is the opposite of the rule above.

### D2. `IntensityControl` works in whole percentages, over a range it is handed

`features/enhancements/IntensityControl.tsx`, props: `label`, `value` (percent), `min`/`max` (percent, optional until
the catalogue arrives), `mark` (the value to label beneath the track), `onChange` (percent, already inside the range).

- **Percent at the control, unit at the panel.** `LightAdjustmentOptions` converts both ways - `Math.round(bias * 100)`
  in, `percent / 100` out - and multiplies the published range by 100 on the way in. The control knows nothing about
  bias, which is what lets denoise hand it `0..300` with a mark at `100` later.
- **The typed field holds `""` and `"-"`** without writing, and restores the carried value on blur. Anything that
  parses as an integer is clamped into the range and written. This is `ScaleControl`'s rule for its own intermediate
  states, applied to the two a signed integer has.
- **The slider writes on `onValueCommit`**, keeping its moving position in local state that the typed field also
  reads. The reference's `ValueSlider` does the same (`onChangeCommitted`), and here it matters more than it did
  there: every write re-runs the preview, and a run superseded mid-flight is a model call thrown away.
- **Step 5 on the slider, any integer typed**: the reference's `IntensitySelector` values, kept.
- **`track={false}`**, because the value is about a neutral zero, not a quantity from the left edge - the case the
  prop was added for.
- **No bounds yet, no slider.** Before the catalogue arrives the slider is disabled and the field does not clamp,
  as `ScaleControl` renders Max unavailable without a range. Inventing `-100..100` here would be the interface keeping
  its own copy of a range the application publishes.

*Alternative considered:* a control generic over unit values with a `scale` prop. Rejected: every intensity family
presents a percentage, so the conversion would be the same `* 100` in every caller's props rather than in one place
per family's panel - and the panel is where the family's parameter is already named.

### D3. The label is the caller's, and this one says "Bias"

`IntensityControl` takes its label as a string. `LightAdjustmentOptions` passes `t("enhancements.bias")`, a new key in
all thirteen locales. The existing `enhancements.intensity` stays for the families the design labels that way.

### D4. The bounds come from the selected variant's published `bias` parameter

Found by name (`"bias"`, the library's `Bias::NAME`) on the **selected** variant, as `UpscaleOptions` finds `scale` -
per model rather than per family, because `VariantEntry.parameters` is per model.

### D5. A new light adjustment arrives at 0.5

`newOperation`'s `light_adjustment` arm returns `{ family, codename, precision, bias: DEFAULT_BIAS }` with
`DEFAULT_BIAS = 0.5`, the reference's `defaultAmount`. A constant in `lib/enhancements.ts` beside `defaultScale`: which
amount a freshly added enhancement starts at is a presentation decision, the same one `add-gui-enhance-2-sidebar`
made for the scale, and `autopilot.rs` already declines to hold defaults for a front end.

The default model needs nothing: `modelChoice` falls back to the catalogue's first variant, and the catalogue lists
Paris first, which is also the reference's `defaultModel`.

### D6. Where the work sits

| Crate | What |
|---|---|
| `opai` | Nothing. Everything this change calls is published and tested. |
| `gui` (Rust) | The `LightAdjustment` arm of `Requested` and its tests. |
| `gui` (frontend) | The union member, `RUNNABLE`, `newOperation`, `IntensityControl`, `LightAdjustmentOptions`, the row's two switches, the locale key. |
| `cli` | Nothing. The CLI already reaches light adjustment through `opai`. |

`rust-sak`: no feature used, and no gap - the change contains no infrastructure. No crate or package is added.

### Sessions, providers and platforms

No session, memory budget or provider code changes. A light adjustment is opened, cached and released by the same
`Opai::execute` path an upscale is, under Paris' and Lyon's existing `EpProfile`s. The change is validated on macOS
(CPU and CoreML through the existing profiles, via the GUI run). Windows and Linux are untested here; nothing in the
change is platform-specific.

## Slices

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-light-adjustment` | The `light_adjustment` wire arm; Light Adjustment in the add menu at a 50% bias; the `Paris, 25%, HD` info line; the options panel with the model tray and the Bias intensity control; the `enhancements.bias` key | - | **current** |

**This is a new series**, named after the feature. The `add-faces-*` series closed on its second slice, and
`add-light-adjustment-paris-lyon` was the library-side change for this family, not a slice of a GUI series.

## Risks / Trade-offs

- **[Typing `25` writes `2`, then `25`]** → two runs, the first stopped by the second. Accepted: it is what typing a
  scale does today, and the stop rule already guarantees the superseded run produces no image. Deferring every typed
  write to blur would make the field feel dead.
- **[A control with one caller built for four]** → kept to props the design already specifies for the others (screen
  12's range and mark). Nothing speculative - no per-family formatter, no second label slot.
- **[Rounding]** → `Bias` quantises to 0.001 and the control shows whole percentages, so a bias written elsewhere at
  0.255 reads `26%` (`Math.round`). Nothing in this window writes a non-integer percentage, so the only source would
  be a future one; the row and the control round the same way, which keeps them agreeing.

## Migration Plan

Nothing to migrate. `Requested` gains an arm, which is additive on the wire; no stored preference changes shape, and
the default-model preference for light adjustment already exists in the Settings dialog.
