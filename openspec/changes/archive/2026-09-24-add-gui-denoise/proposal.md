## Why

Stockholm, Gothenburg and Malmö run in `opai`, and the catalogue publishes all three along with their `strength` range -
but the window cannot ask for any of them. `Requested` in `crates/gui/src/enhance/operation.rs` has four arms,
`upscale`, `face_recovery`, `light_adjustment` and `color_balance`, so a denoise is refused at parse time, and the add
menu offers exactly what the wire can carry. Denoise is the fifth family to reach the window and the first in pipeline
order.

Its options are the Light Adjustment and Color Balance panel with different numbers - screen 12 of the design draws the
same model tray, separator and typed-percentage slider. Those two panels are already line-for-line copies of each
other, kept apart on purpose by `add-gui-color-balance` (design D3), which named "a third family" as the moment to
revisit that. Denoise is that third family, so this change folds the three into one shared panel.

## What Changes

This is a **single-slice change**. It delivers Denoise in the add menu, in the sidebar with its info line, and in an
options panel holding the Stockholm / Gothenburg / Malmö model tray and a Strength control - and it moves the two bias
panels onto the same shared component.

- **A `denoise` arm on the wire operation**, carrying the codename, the precision and the strength as a unit value
  (0..3). A strength outside the range is **brought inside it and logged** rather than refused, the rule the bias arms
  already follow.
- **Denoise joins the add menu** - the fifth member of `RUNNABLE`, and the first in pipeline order, before Face
  Recovery. It is added carrying the user's default model and a **strength of 100%**, which is the model's own output.
- **Its row reports `Stockholm, 100%, HD`** through the `enhancements.info` sentence every locale already carries.
- **The options panel**: the model tray, a separator, and `IntensityControl` labelled **Strength**, running over the
  selected model's published range (0..300%) with a mark at **100**.
- **One shared panel, `IntensityOptions`**, holding the tray, the separator, the control, the lookup of the selected
  model's range and the unit/percent conversion. `LightAdjustmentOptions`, `ColorBalanceOptions` and the new
  `DenoiseOptions` stay as the per-family entry points, and each only names its parameter, its label and its mark. The
  range is **not** a prop: it comes from the catalogue, found by the parameter's name.
- **One new i18n key**, `enhancements.strength`, in all thirteen locales. `enhancements.denoise.name` and
  `preview.progress.denoise` are already there.

**Deliberate divergences from the reference's behaviour:**

- **The control is labelled "Strength", not "Intensity".** The reference's `OptionsDenoise` passes no label, so its
  `IntensitySelector` shows "Intensity". Screen 12 names it "Strength", and the parameter is `opai::Strength`.

**Not in this change:**

- **Sharpen.** It takes the same `opai::Strength` and will be one more wrapper around `IntensityOptions`, but its wire
  arm, its default and its label are that change's decisions. The design leaves Sharpen on "Intensity"; the rule this
  change and colour balance follow (the label follows the parameter type) would say "Strength", and that is where the
  question should be settled.
- **The popover does not open by itself when Denoise is added**, for the reason recorded in `add-gui-light-adjustment`.
- **Autopilot.** The window calls no autopilot yet.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-enhance`: a denoise can be asked for over the wire with its strength brought inside the permitted range; the
  add menu offers it first and adds it at a strength of 100%; its row reports the strength as a percentage; its options
  offer the strength as a typed percentage and a slider bounded by the published range, labelled Strength and marked
  at 100. The light-adjustment and colour-balance requirements do not change - their panels move onto the shared
  component without any visible difference.

## Impact

- **Crates: `gui` only.** `opai` already publishes, builds and runs all three variants - `VariantEntry::build` resolves
  a denoise from a codename, a precision and `ParameterValues::with_strength` (`build.rs`), and the catalogue publishes
  `strength` as `0..3` for the family. Nothing here would be needed by `cli`: the wire shape, the default of 100% and
  the percentage presentation are this window's decisions about presentation.
- **Rust:** `crates/gui/src/enhance/operation.rs` gains the `Denoise` arm and its tests.
- **Frontend:** `ipc/enhance.ts` (the union member), `lib/enhancements.ts` (`RUNNABLE`, `newOperation`, a
  `DEFAULT_STRENGTH`), `features/enhancements/` (a new `IntensityOptions` and `DenoiseOptions`, the two bias panels
  reduced to wrappers, the row's info and options switches) and their tests.
- **i18n:** `enhancements.strength` in all thirteen locales.
- **`rust-sak`:** nothing needed; the change has no infrastructure.
- **No new crates or packages.**
- **Wire compatibility:** additive. No existing arm, command or stored preference changes shape.
- **Estimated tasks:** 14.
