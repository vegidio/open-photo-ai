## Why

`add-gui-6-settings-ipc` put the three facts the Settings dialog is drawn from on the wire - the provider report,
the log file's location and the model catalogue - and deliberately drew nothing with them. This is the slice that
draws them: screens 16 and 16b, the dialog the navbar's Settings button has been rendering inert since slice 2.

It is also where the application first remembers a preference that is not a single boolean. Thirteen language
catalogues have shipped since slice 1 with no way to pick one; every one of the fifteen settings rows the design
draws - language, logs, analytics, the AI processor, seven default models and four export qualities - is a
preference the rest of the application will read, and none of them has anywhere to live yet.

## What Changes

- **A settings store**, persisted, with the Cancel/Save semantics the design's note requires: *"Language and
  processor changes are drafts until Save."* Every row writes the store as the user touches it, Cancel restores the
  snapshot taken when the dialog opened, and Save is what lets a draft escape - which for the language means this is
  the moment `i18n.changeLanguage` runs.
- **The Settings dialog itself**: the 768x640 card, the reusable 48px dialog title bar with its close box, the nav
  list on the left, the scrolling sections on the right and the Cancel/Save footer. The title bar is built as its own
  component because the crop, export, faces and About dialogs all draw the same one.
- **All fifteen rows**, in four sections, over four row shapes - select, switch, button and slider:
  - **Application**: Language (thirteen endonyms), Logs (Show logs, over slice 6's two commands) and Analytics.
  - **Performance**: AI Processor, its options built from the provider report `initialize` resolved with.
  - **Enhancements**: the seven default-model selects, built from the catalogue, each model offered at both of the
    quality tiers its published precisions define.
  - **Export**: one quality slider per lossy encoder, each with its own default mark and remembered value.
- **The application's language is chosen rather than fixed.** `i18n/index.ts` currently pins `lng` to English with a
  comment saying the store that would replace it has not been written. It is written here: first launch detects from
  the browser's language list, every later launch reads the persisted choice.
- **Toasts**, through `sonner`. The Show logs button is the one control in this dialog whose whole purpose is to help
  a user file a bug report, so a failure that passes silently leaves them with no file and no reason. The catalogues
  have carried `errors.showLogsFailed` since slice 1; this is what shows it.
- **The analytics switch ships with nothing behind it**, persisting the choice and no more. Nothing in this
  application sends an event yet, and the slice that does reads what the user already chose.

No new Rust. Every fact this dialog needs was put on the wire by slice 6, which is what that slice was for.

- **One label corrected in `opai`, after the fact.** `Malmö`'s row published its label as `Malmo`, and its own doc
  said the reference spells it both ways and that "the two are worth reconciling when a front end is wired up". This
  slice is that front end, and drawing the model list is what put the unaccented spelling on screen. Corrected where
  the vocabulary lives rather than patched in the frontend, which is the whole point of the catalogue. The codename
  stays `malmo` - it names artifacts, URLs and cache directories - so nothing composed from it moves.

## Capabilities

### New Capabilities

None. `gui-settings` was created by slice 6 for exactly this.

### Modified Capabilities

- `gui-settings`: adds what the settings *surface* does, beside the three facts it is drawn from. That the
  application has a settings dialog reachable from the navbar; that it holds the fifteen preferences the design
  lists; that a change is a draft until Save and is undone by Cancel; that a preference which the machine or the
  library no longer supports is replaced rather than kept; and that failing to show the log file is reported to the
  user rather than passing silently.

## Impact

**Crates: `gui`, plus one label in `opai`.** The slice was scoped as frontend-only and is otherwise exactly that. The
exception is the `Malmö` label above, which is model vocabulary and belongs in the library by the same argument that
keeps every other model name out of the frontend. No Tauri command is added or widened,
no capability is granted and `Cargo.toml` is untouched. `cli` is not touched at all.

Nothing is put in a front end that belongs in `opai`. What this slice adds is a dialog, a persisted preference and
the wiring between them - a choice a user makes about this application, which `cli` has no use for and `opai` has
nowhere to read. The library's own position is already recorded in `autopilot.rs`: a suggestion *"names the family
and whatever the pixels determine about it... and never a model variant or a precision. Those are a standing user
preference, and `opai` has nowhere to read one from."* This slice is where that preference is finally stored.

**rust-sak: nothing used and nothing missing.** There is no Rust logic in this slice and no infrastructure in it - no
transfer, no hash, no extraction, no probe, no cache. The one thing that reaches the filesystem, revealing the log
file, is slice 6's command and is already built on `rust-sak`'s non-creating `fs::user_config_dir` inside `opai`.

**New npm dependencies: one, plus two generated components.** `sonner` for the toaster, added through
`shadcn add sonner`, alongside `shadcn add select`. Both land as source in `components/ui/` and get the hand-edits
every `shadcn add` in this crate needs; see design.md. No Rust crate is added.

**Task estimate: fifteen.** At the limit, with no slack. Slice 6's design scoped this at eleven, on the assumption
that a `default` model would be published by `opai`; the decision in design.md removes that `opai` change, and the
fifteen are all frontend. Where it overruns, the compression point is named in design.md.

**Parity: one deliberate divergence, and one different surface for the same failure.**

- **Upscale defaults to Tokyo, not Kyoto.** The reference states a default model per enhancement in its frontend
  registry and notes that *"presentation order and the sensible default are not the same thing: upscale lists Tokyo
  first but defaults to Kyoto."* This slice takes the default from the catalogue's own published order instead - the
  first variant a family offers - which agrees with the reference for six of the seven families and differs for
  upscale. The gap is worth it because the alternative is one of two things this project has already argued against:
  seven model codenames written into a front end, which is what the catalogue exists to prevent, or a `default` field
  added to `opai`'s catalogue so the library states a preference it has just been quoted as refusing to hold. Tokyo
  is a defensible first upscale in its own right - it is the conservative one, and the catalogue already lists it
  first for that reason.
- **A failed Show logs is a toast rather than a snackbar.** Same sentence, same moment, a different component -
  which is an implementation difference and not a behavioural one. It is noted only because it brings in the
  application's first toaster, which later slices will use for `toasts.unsupportedFiles` and the CPU-fallback notice.

The reference's first-run TensorRT dialog (`dialogs.tensorRT.*` and the `isFirstTensorRT` flag beside it in its
settings store) is **not** in this slice. It is not a settings row, no screen in the design draws it, and the slices
table has not cut it.
