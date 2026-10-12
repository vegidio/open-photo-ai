## Why

Delhi, Mumbai and Jaipur run in `opai` since `add-colorization-3-pipeline`, and the catalogue publishes all three with
no parameters - but the window cannot ask for any of them. `Requested` in `crates/gui/src/enhance/operation.rs` has six
arms and no `colorization`, so a colorization is refused at parse time, and the add menu leaves it out because
`RUNNABLE` does. Colorization is the seventh and last enhancement family to reach the window.

It is also the one that closes the gap the menu's filter was built for. `RUNNABLE` exists to say "the menu offers what
the wire can carry", which is a statement about work in progress; with colorization on the wire it filters nothing out.
The same goes for the refused-family test, which has used whichever family was still missing as its example.

## What Changes

This is a **single-slice change**. It delivers Colorization in the add menu, in the sidebar with its info line, and in
an options panel holding the Delhi / Mumbai / Jaipur model tray and nothing else.

- **A `colorization` arm on the wire operation**, carrying the codename and the precision only. Colorization takes no
  parameter, so there is nothing to clamp and no value to carry.
- **Colorization joins the add menu** between Face Recovery and Light Adjustment, which is pipeline order. It is added
  carrying the user's default model, which falls back to Delhi, the catalogue's first and the reference's default.
- **Its row reports `Delhi, HD`** through the `enhancements.infoModel` sentence every locale already carries.
- **The options panel is the model tray alone**: a `ColorizationOptions` holding `ModelTray`, no separator and no
  amount control - the reference's `OptionsColorization`, drawn with this application's shadcn components.
- **The menu's filter is removed.** `RUNNABLE`, its `Unoffered` compile check and `RUNNABLE_ENHANCEMENTS` are deleted;
  the menu draws `ENHANCEMENTS` directly. The guarantee that every family the wire carries is offered is kept as a
  compile check on `ENHANCEMENTS` itself.
- **The refused-family test and its spec scenario are deleted.** Every family the window presents now has an arm, so
  the test has no family left to use as its example. The refusal of an unpublished model or precision, and the rule
  that the whole chain is judged before any of it runs, stay and keep their tests.
- **No new i18n keys.** `enhancements.colorization.*`, `preview.progress.colorization` and `enhancements.infoModel` are
  already in all thirteen locales.

**Deliberate divergences from the reference's behaviour:** none. The panel, the default model, the info line and the
menu position are the reference's. Removing the menu filter ends the one divergence the add menu had from screen 10 of
the design, which draws all seven enhancements.

**Not in this change:**

- **The popover does not open by itself when Colorization is added**, for the reason recorded in
  `add-gui-light-adjustment`.
- **Autopilot.** The window calls no autopilot yet, although `opai`'s autopilot can now suggest colorization.
- **Any warning that colorization replaces a colour photograph's colour.** The reference has none, and the model
  descriptions already frame the family as being for black-and-white photographs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-enhance`: a colorization can be asked for over the wire by its model and precision alone; the add menu offers
  every enhancement the application presents, with colorization between face recovery and light adjustment; its row
  reports the model and quality; its options offer the model and nothing else. The requirement that refuses what the
  application cannot run is replaced by one about what it does not publish, dropping the scenario about a family the
  application cannot run.

## Impact

- **Crates: `gui` only.** `opai` already publishes, builds and runs all three variants - `VariantEntry::build` resolves
  a colorization from a codename and a precision (`build.rs`), and the catalogue publishes no parameters for the
  family. Nothing here would be needed by `cli`: the wire shape, the menu and the panel are this window's decisions
  about presentation.
- **Rust:** `crates/gui/src/enhance/operation.rs` gains the `Colorization` arm and its tests, loses the refused-family
  test, and its docs stop describing a family without an arm as a case that arrives.
- **Frontend:** `ipc/enhance.ts` (the union member), `lib/enhancements.ts` (`ENHANCEMENTS` typed so the compile check
  can read it, `RUNNABLE` and its two companions deleted, `newOperation`), `features/enhancements/` (a new
  `ColorizationOptions`, the row's info and options switches, `AddEnhancement` drawing `ENHANCEMENTS`) and their tests.
- **i18n:** nothing.
- **`rust-sak`:** nothing needed; the change has no infrastructure.
- **No new crates or packages.**
- **Wire compatibility:** additive. No existing arm, command or stored preference changes shape.
- **Estimated tasks:** 12.
