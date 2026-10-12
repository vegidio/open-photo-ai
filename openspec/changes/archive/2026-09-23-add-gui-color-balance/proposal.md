## Why

Rio and São Paulo run in `opai`, and the catalogue publishes both along with their `bias` range - but the window
cannot ask for either. `Requested` in `crates/gui/src/enhance/operation.rs` has three arms, `upscale`, `face_recovery`
and `light_adjustment`, so a colour balance is refused at parse time, and the add menu offers exactly what the wire can
carry. Colour balance is the fourth family to reach the window, and the second to take a bias: it takes the same
`opai::Bias` a light adjustment does, over the same -1..1 range, so everything `add-gui-light-adjustment` built for its
amount - the intensity control, the unit-to-percent conversion, the clamp on the wire - is already there to be used.

## What Changes

This is a **single-slice change**. It delivers Color Balance in the add menu, in the sidebar with its info line, and in
an options panel holding the Rio / São Paulo model tray and a bias control - the same UI Light Adjustment has.

- **A `color_balance` arm on the wire operation**, carrying the codename, the precision and the bias as a unit value
  (-1..1). A bias outside the range is **brought inside it and logged** rather than refused, which is the rule the
  light-adjustment arm already follows.
- **Color Balance joins the add menu** - the fourth member of `RUNNABLE`, slotted by `ENHANCEMENTS` into pipeline order
  between Light Adjustment and Upscale. It is added carrying the user's default model and a **bias of 50%**.
- **Its row reports `Rio, 50%, HD`** through the `enhancements.info` sentence every locale already carries.
- **The options panel**: the model tray, a separator, and the existing `IntensityControl` labelled **Bias**, running
  over the selected model's published range with a mark at 0. A panel of its own, `ColorBalanceOptions`, beside
  `LightAdjustmentOptions` rather than shared with it.
- **No new i18n keys.** `enhancements.colorBalance.name`, `preview.progress.colorBalance` and `enhancements.bias` are
  already in all thirteen locales.

**Deliberate divergences from the reference's behaviour:**

- **The control is labelled "Bias", not "Intensity".** The reference labels it "Intensity", and so does screen 12 of the
  design - by default rather than by choice: the design gives Light Adjustment `paramLabel: 'Bias'` and gives Color
  Balance no label, so it falls back to "Intensity". Colour balance takes the same `opai::Bias` light adjustment does,
  -1..1 about a neutral 0, which says *which direction and how far*; "Intensity" says *how much*. The two families
  carry one parameter type and now carry one label for it.
- **A half-typed value runs nothing**, as for light adjustment: an emptied field or a lone `-` is held rather than
  written as a bias of 0. This comes with `IntensityControl` and needs no new work.

**Not in this change:**

- **The popover does not open by itself when Color Balance is added**, for the reason recorded in
  `add-gui-light-adjustment`: adding an enhancement keeps today's behaviour for every family.
- **Autopilot.** The window calls no autopilot yet.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-enhance`: a colour balance can be asked for over the wire with its bias brought inside the permitted range; the
  add menu offers it and adds it at a bias of 50%; its row reports the bias as a percentage; its options offer the bias
  as a typed percentage and a slider bounded by the published range, labelled Bias.

## Impact

- **Crates: `gui` only.** `opai` already publishes, builds and runs both variants - `VariantEntry::build` resolves a
  colour balance from a codename, a precision and `ParameterValues::with_bias` (`build.rs`), and the catalogue publishes
  `bias` as `-1..1` for the family. Nothing here would be needed by `cli`: the wire shape, the default of 50% and the
  percentage presentation are all this window's decisions about presentation.
- **Rust:** `crates/gui/src/enhance/operation.rs` gains the `ColorBalance` arm and its tests.
- **Frontend:** `ipc/enhance.ts` (the union member), `lib/enhancements.ts` (`RUNNABLE`, `newOperation`),
  `features/enhancements/` (a new `ColorBalanceOptions`, the row's info and options switches) and their tests.
- **i18n:** none; every key used is already in all thirteen locales.
- **`rust-sak`:** nothing needed; the change has no infrastructure.
- **No new crates or packages.**
- **Wire compatibility:** additive. No existing arm, command or stored preference changes shape.
