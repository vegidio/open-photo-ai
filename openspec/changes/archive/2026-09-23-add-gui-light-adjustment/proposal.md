## Why

Paris and Lyon run in `opai` (`add-light-adjustment-paris-lyon`), and the catalogue publishes both along with their
`bias` range - but the window cannot ask for either. `Requested` in `crates/gui/src/enhance/operation.rs` has two arms,
`upscale` and `face_recovery`, so a light adjustment is refused at parse time, and the add menu offers exactly what the
wire can carry. Light adjustment is the third family to reach the window and the first whose amount is an
**intensity** rather than a scale or a set of faces, so it is also where screen 12's shared intensity control gets
built.

## What Changes

This is a **single-slice change**. It delivers screen 11b of the design: Light Adjustment in the add menu, in the
sidebar with its info line, and in an options panel holding the Paris / Lyon model tray and a bias control.

- **A `light_adjustment` arm on the wire operation**, carrying the codename, the precision and the bias as a unit
  value (-1..1, what `opai::Bias` takes). A bias outside the range is **brought inside it and logged** rather than
  refused, which is the rule a scale already follows and for the same reason: the control that drives it is bounded.
- **Light Adjustment joins the add menu** - the third member of `RUNNABLE`, slotted by `ENHANCEMENTS` into pipeline
  order between Face Recovery and Upscale. It is added carrying the user's default model and a **bias of 50%**.
- **Its row reports `Paris, 25%, HD`** through the `enhancements.info` sentence every locale already carries.
- **The options panel**: the model tray every family shares, a separator, and an **intensity control** - a typed
  percentage beside a slider running over the published range with a mark at 0 - labelled **Bias**. The slider
  writes the enhancement when it is released, not on every step of a drag, so a drag asks for one run rather than
  dozens.
- **A new `enhancements.bias` key in all thirteen locales.**

The control is built to take its range and its mark from its caller, because screen 12 names it as the one denoise,
sharpen and colour balance share. Nothing else here anticipates those families: none of them joins the wire or the
menu.

**Deliberate divergences from the reference's behaviour:**

- **The control is labelled "Bias", not "Intensity".** The reference labels all four intensity families
  "Intensity". The design (screen 11b) labels this one Bias, which is also the library's own name for the parameter:
  `opai::Bias` is documented as a separate type from `Strength` precisely because -1..1 about 0 says *which direction
  and how far*, where 0..3 about 1 says *how much*. The label now says the same thing the type does.
- **A half-typed value runs nothing.** The reference writes a bias of 0 - and so starts a run - the moment the field
  is emptied or holds only `-`, both of which are keystrokes on the way to a negative number. Here both are held, as
  `ScaleControl` already holds an empty field or a trailing separator, and leaving the field settles them.

**Not in this change:**

- **The popover does not open by itself when Light Adjustment is added.** Screen 11b's note says it does; that note
  is wrong, and adding an enhancement keeps today's behaviour for every family.
- **Autopilot.** `opai` now detects when a photograph wants a light adjustment, but the window calls no autopilot
  yet; screen 11b shows autopilot's suggestions only as the stack it starts from.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-enhance`: a light adjustment can be asked for over the wire with its bias brought inside the permitted range;
  the add menu offers it and adds it at a bias of 50%; its row reports the bias as a percentage; its options offer
  the bias as a typed percentage and a slider bounded by the published range.

## Impact

- **Crates: `gui` only.** `opai` already publishes, builds and runs both variants - `VariantEntry::build` resolves a
  light adjustment from a codename, a precision and `ParameterValues::with_bias`, and `Bias::clamped` is documented as
  what a GUI slider wants. Nothing in this change would be needed by `cli`: the wire shape, the default of 50% and the
  percentage presentation are all this window's decisions about presentation.
- **Rust:** `crates/gui/src/enhance/operation.rs` gains the `LightAdjustment` arm and its tests.
- **Frontend:** `ipc/enhance.ts` (the union member), `lib/enhancements.ts` (`RUNNABLE`, `newOperation`),
  `features/enhancements/` (a new `IntensityControl`, a new `LightAdjustmentOptions`, the row's info and options
  switches), and all thirteen `i18n/locales/*.json`.
- **`rust-sak`:** nothing needed; the change has no infrastructure.
- **No new crates or packages.** The slider is the existing `components/ui/slider.tsx`, whose `track={false}` was
  added for a slider about a meaningful zero.
- **Wire compatibility:** additive. No existing arm, command or stored preference changes shape.
