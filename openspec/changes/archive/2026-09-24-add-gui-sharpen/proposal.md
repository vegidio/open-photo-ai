## Why

Moscow, Petersburg and Novgorod run in `opai` since `add-sharpen-moscow-petersburg-novgorod`, and the catalogue
publishes all three with their `strength` range - but the window cannot ask for any of them. `Requested` in
`crates/gui/src/enhance/operation.rs` has five arms and no `sharpen`, so a sharpen is refused at parse time, and a test
there uses sharpen as its example of a family the window cannot run. Sharpen is the sixth family to reach the window.

Its options are Denoise's panel: the same model tray, separator and typed-percentage slider over the same `opai::Strength`
(0..300%, marked at 100). `add-gui-denoise` built `IntensityOptions` so that this family would be one more thin wrapper,
and deferred to this change the one question it could not settle - what the control is called.

## What Changes

This is a **single-slice change**. It delivers Sharpen in the add menu, in the sidebar with its info line, and in an
options panel holding the Moscow / Petersburg / Novgorod model tray and a Strength control.

- **A `sharpen` arm on the wire operation**, carrying the codename, the precision and the strength as a unit value
  (0..3). A strength outside the range is **brought inside it and logged** rather than refused, the rule the denoise
  and bias arms already follow.
- **Sharpen joins the add menu** - the sixth member of `RUNNABLE`, after Color Balance and before Upscale, which is
  pipeline order. It is added carrying the user's default model and a **strength of 100%**, the model's own output.
- **Its row reports `Moscow, 100%, HD`** through the `enhancements.info` sentence every locale already carries.
- **The options panel**: a `SharpenOptions` wrapper around the shared `IntensityOptions` - parameter `strength`, label
  **Strength**, mark at **100** - over the selected model's published range.
- **The refused-family test moves to colorization**, the one family the window still cannot run.
- **No new i18n keys.** `enhancements.sharpen.*`, `preview.progress.sharpen` and `enhancements.strength` are already in
  all thirteen locales.

**Deliberate divergences from the reference's behaviour:**

- **The control is labelled "Strength", not "Intensity".** The reference's `OptionsSharpen` passes no label, so its
  `IntensitySelector` shows "Intensity", and the design's screen follows it. The parameter is `opai::Strength`, the
  rule set by `add-gui-color-balance` (the label follows the parameter type) gives "Strength", and Denoise - the same
  parameter, range and mark - already says "Strength". Two panels differing only in that word would say two different
  things are being set. Decided by the user while exploring this change.

**Not in this change:**

- **Colorization.** It is the last family without a wire arm; its options are not a single amount and are its own change.
- **The popover does not open by itself when Sharpen is added**, for the reason recorded in `add-gui-light-adjustment`.
- **Autopilot.** The window calls no autopilot yet.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-enhance`: a sharpen can be asked for over the wire with its strength brought inside the permitted range; the add
  menu offers it between colour balance and upscale and adds it at a strength of 100%; its row reports the strength as a
  percentage; its options offer the strength as a typed percentage and a slider bounded by the published range,
  labelled Strength and marked at 100.

## Impact

- **Crates: `gui` only.** `opai` already publishes, builds and runs all three variants - `VariantEntry::build` resolves
  a sharpen from a codename, a precision and `ParameterValues::with_strength` (`build.rs`), and the catalogue publishes
  `strength` as `0..3` for the family. Nothing here would be needed by `cli`: the wire shape, the default of 100% and the
  percentage presentation are this window's decisions about presentation.
- **Rust:** `crates/gui/src/enhance/operation.rs` gains the `Sharpen` arm and its tests; the refused-family test names
  colorization instead.
- **Frontend:** `ipc/enhance.ts` (the union member), `lib/enhancements.ts` (`RUNNABLE`, `newOperation`),
  `features/enhancements/` (a new `SharpenOptions`, the row's info and options switches), `test/support.tsx` (the
  sharpen fixture gains its `strength` parameter) and their tests.
- **i18n:** nothing.
- **`rust-sak`:** nothing needed; the change has no infrastructure.
- **No new crates or packages.**
- **Wire compatibility:** additive. No existing arm, command or stored preference changes shape.
- **Estimated tasks:** 11.
