## Context

See proposal.md - Why. The state this design builds on:

- **`opai` is finished for this family.** `VariantEntry::published(Family::ColorBalance, codename)?.build(precision,
  &values)` resolves Rio or São Paulo (`build.rs`), `ParameterValues::with_bias` carries the amount, and the catalogue
  publishes one parameter for the family, `bias`, as `Range { min: -1.0, max: 1.0 }`, per variant, with Rio listed
  first.
- **The wire has three arms.** `Requested` in `crates/gui/src/enhance/operation.rs` is tagged by family; its
  `LightAdjustment` arm already clamps and logs a bias. `ipc/enhance.ts`'s `Operation` union mirrors it, and
  `lib/enhancements.ts`'s `Unoffered` check fails the build if `RUNNABLE` omits a member of that union.
- **`IntensityControl` exists**, built by `add-gui-light-adjustment` to take its label, range and mark from its caller
  so that the other intensity families could be handed it unchanged. This is its second caller.
- **`LightAdjustmentOptions`** is the panel this one matches: `ModelTray`, `Separator`, `IntensityControl`, with the
  bounds read from the selected variant's `bias` parameter and the percent/unit conversion done in the panel.
- **Every locale already carries** `enhancements.colorBalance.name`, `preview.progress.colorBalance`,
  `enhancements.info` and `enhancements.bias`. `ENHANCEMENTS` already lists colour balance, with its `Palette` icon,
  between light adjustment and sharpen.
- **The reference** (`OptionsColorBalance`) is its `OptionsLightAdjustment` with the family changed: default amount
  0.5, default model `rio`, the same `IntensitySelector`.

## Goals / Non-Goals

**Goals:**

- Colour balance runnable end to end from the window, looking as Light Adjustment's panel does.

**Non-Goals:**

- Denoise or sharpen on the wire or in the menu.
- Opening a panel when its enhancement is added (see `add-gui-light-adjustment`'s proposal).
- Any change to `opai`, to sessions or to execution providers.
- Folding the two bias panels into one (D3).

## Decisions

### D1. The wire carries the library's unit bias, clamped and logged

`Requested::ColorBalance { codename, precision, bias: f64 }`, resolved as
`ParameterValues::new().with_bias(Bias::clamped(bias))` with a `tracing::warn!` when `bias` falls outside
`Bias::MIN..=Bias::MAX` - the `LightAdjustment` arm's shape exactly, for the reasons its D1 records (the wire speaks
the library's vocabulary; a bounded control makes an out-of-range value a fault, better answered with a picture and a
log line than a failed run).

The two arms' bodies differ only in the family they name. They are written out rather than folded into a helper or a
shared arm: serde needs one variant per tag, and a helper taking a `Family` saves four lines while hiding which family
a reader is looking at - the same trade `ColorBalance::pipeline` makes in `opai` for its two arms.

### D2. The label is "Bias"

`ColorBalanceOptions` passes `t("enhancements.bias")`. Screen 12 falls back to "Intensity" for colour balance because
it names no label for it, where it names "Bias" for light adjustment; the parameter both families take is one type,
`opai::Bias`, and the label follows the type. See proposal.md - divergences.

### D3. A panel of its own, not a shared bias panel

`features/enhancements/ColorBalanceOptions.tsx`, beside `LightAdjustmentOptions` and nearly identical to it. The two
could be one component taking a label; they are kept apart by decision, so each family's panel stays where a reader
of that family looks for it and either can change without the other. The file SHALL say so in a comment next to the
copied logic, so that a later cleanup does not fold them without that decision being revisited.

The shared parts are already shared: `ModelTray`, `IntensityControl` and `optionFor`/`modelValue`. What the two panels
repeat is the `bias` parameter lookup and the ×100 conversion - a few lines each.

One change lands in the shared part, and so reaches Light Adjustment's panel too: `IntensityControl`'s neutral mark
moves from 1px to 8px below the slider track, with the bottom margin grown to match (`mb-4` → `mb-[23px]`), because at
1px the `0` sat against the track. It was found and checked in the launch check (task 4.1). It is made in the shared
control rather than in this panel because the two bias panels have to look the same.

### D4. The bounds come from the selected variant's published `bias` parameter

Found by name (`"bias"`) on the **selected** variant, as `LightAdjustmentOptions` does, because
`VariantEntry.parameters` is per model.

### D5. A new colour balance arrives at 0.5, with the catalogue's first model

`newOperation`'s `color_balance` arm returns `{ family, codename, precision, bias: DEFAULT_BIAS }`, reusing the
existing constant: the reference's `defaultAmount` is 0.5 for both families. `DEFAULT_BIAS`'s doc widens from "a
freshly added light adjustment" to both bias families.

The default model needs nothing: `modelChoice` falls back to the catalogue's first variant, which is Rio - also the
reference's `defaultModel`.

### D6. Where the work sits

| Crate | What |
|---|---|
| `opai` | Nothing. Everything this change calls is published and tested. |
| `gui` (Rust) | The `ColorBalance` arm of `Requested` and its tests. |
| `gui` (frontend) | The union member, `RUNNABLE`, `newOperation`, `ColorBalanceOptions`, the row's two switches, tests. |
| `cli` | Nothing. The CLI already reaches colour balance through `opai`. |

`rust-sak`: no feature used, and no gap - the change contains no infrastructure. No crate or package is added.

### Sessions, providers and platforms

No session, memory budget or provider code changes. A colour balance is opened, cached and released by the same
`Opai::execute` path a light adjustment is, under Rio's and São Paulo's existing `EpProfile`s. The change is validated
on macOS (CPU and CoreML through the existing profiles, via the GUI run). Windows and Linux are untested here; nothing
in the change is platform-specific.

## Slices

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-color-balance` | The `color_balance` wire arm; Color Balance in the add menu at a 50% bias; the `Rio, 50%, HD` info line; the options panel with the model tray and the Bias intensity control | - | **current** |

**This is a new series**, named after the feature. `add-gui-light-adjustment` was a series of one and is closed.

## Risks / Trade-offs

- **[Two near-identical panels]** → accepted by decision (D3); the comment in the file is what keeps it deliberate
  rather than accidental. A third bias family would be the moment to revisit it.
- **[Typing `25` writes `2`, then `25`]** → two runs, the first stopped by the second - inherited from
  `IntensityControl`, accepted there for the same reasons.
- **[The label departs from the design]** → one string; if the design is later updated to say "Intensity" on purpose,
  the change is the key passed in `ColorBalanceOptions`, and `enhancements.intensity` already exists in every locale.

## Migration Plan

Nothing to migrate. `Requested` gains an arm, which is additive on the wire; no stored preference changes shape, and
the default-model preference for colour balance already exists in the Settings dialog.
