## Context

See proposal.md - Why. The state this design builds on:

- **`opai` is finished for this family.** `VariantEntry::published(Family::Colorization, codename)?.build(precision,
  &values)` resolves Delhi, Mumbai or Jaipur (`build.rs`) to `Colorization::delhi`, `::mumbai` or `::jaipur`. The
  catalogue publishes the family with an empty parameter list (`simple_family`), and `colorization::ALL` lists Delhi
  first. What a run does to a photograph is the `colorization` capability's contract.
- **The wire has six arms.** `Requested` in `crates/gui/src/enhance/operation.rs` is tagged by family. Its docs say a
  family with no arm "is refused at parse time", and `a_family_this_application_cannot_run_is_refused_by_the_parse_and_names_it`
  sends a colorization as its example. The whole-chain rule is tested separately, by
  `an_operation_the_application_cannot_run_is_refused_by_position` in `enhance/run.rs`, through an unpublished upscale
  model.
- **The menu is filtered.** `lib/enhancements.ts` has `ENHANCEMENTS` (seven entries, pipeline order, typed
  `readonly Enhancement[]`), `RUNNABLE` (six families), an `Unoffered` compile check that every `Operation` family is
  in `RUNNABLE`, and `RUNNABLE_ENHANCEMENTS`, which filters the first by the second and narrows `family` to
  `Operation["family"]`. `AddEnhancement` draws `RUNNABLE_ENHANCEMENTS` and records the filter as a divergence from
  screen 10 of the design. `RUNNABLE` has no other reader outside its tests.
- **Everything the frontend shows is already written.** `ENHANCEMENTS` carries a `colorization` entry (Brush icon)
  between face recovery and light adjustment. Every locale carries `enhancements.colorization.name`, the three model
  descriptions, `preview.progress.colorization` and `enhancements.infoModel` (`{{name}}, {{quality}}`). The Settings
  dialog already stores a default colorization model, and `test/support.tsx` already lists the three variants with no
  parameters, which is the real catalogue's shape.
- **The reference** (`OptionsColorization`) is `OptionsPopover` around a `ModelSelector` alone. Its comment: "Colorization
  has no per-run amount, so only the model half of the hook is used." `ENHANCEMENTS.cl` has `infoKey:
  'enhancements.infoModel'` and `defaultModel: 'delhi'`.

## Goals / Non-Goals

**Goals:**

- Colorization runnable end to end from the window, its panel the reference's model tray.
- No code left describing a family the window cannot ask for.

**Non-Goals:**

- Any change to `ModelTray`, `OptionsPanel`, `opai`, sessions or execution providers.
- Autopilot in the window.

## Decisions

### D1. The wire arm carries the model and nothing else

`Requested::Colorization { codename, precision }`, resolved as `VariantEntry::published(Family::Colorization,
..).build(precision, &ParameterValues::new())`. There is no value to clamp and no warning to log.

**Not a `values` field kept empty "for later".** A field describing a choice the user cannot make is what
`FaceRecovery` already declines for fidelity (`add-faces-1-seam` D7), and a colorization parameter arriving later would be
its own change to the arm.

### D2. The refused-family test is deleted, not moved

Decided by the user while exploring this change. The test existed to show a family without an arm is refused by the
parse. Every family the window presents now has one, and the only published family left without an arm is
`detection`, which the window never sends as an operation. Moving the test there would pin a refusal that no
interface can trigger. The spec requirement is replaced accordingly (see the delta's REMOVED/ADDED pair): the
unpublished model and precision scenarios stay, and the whole-chain scenario is now stated through an unpublished
model, which `an_operation_the_application_cannot_run_is_refused_by_position` already tests.

The `Requested` doc sentence "A family with no arm here is refused at parse time ... a family becomes runnable by
gaining an arm here" and the doc comments that count the families are rewritten to describe seven arms, one per
enhancement the window presents, without the "until it gets one" framing.

### D3. `ColorizationOptions` is the tray alone

`features/enhancements/ColorizationOptions.tsx` renders `ModelTray` with the operation's `modelValue` and, on a choice,
calls `onChange({ ...operation, codename, precision })` through `optionFor`. Same props as every other panel
(`enhancement`, `operation`, `entry`, `onChange`), so `EnhancementRow`'s options switch gains one case. `OptionsPanel`,
drawn by the row, is the frame, as for every other family.

**Not `IntensityOptions` with the amount made optional.** That component is "a model and one amount"
(`add-gui-denoise` D2); making its defining half optional would turn every one of its props into a question. The tray
is already its own component, so this wrapper is the reference's shape with nothing to share.

**No separator, no note.** Decided by the user: the panel matches the Go/Wails version, drawn with this application's
shadcn components. The reference's divider sits between a model selector and a second control; there is no second
control here.

### D4. The row reports `infoModel`

`EnhancementRow`'s info switch gains `case "colorization": return t("enhancements.infoModel", { name, quality })` - the
sentence face recovery already uses before its faces are known, and the reference's `infoKey` for the family. Its own
case rather than a fall-through onto face recovery, whose case computes faces.

### D5. A new colorization carries the default model alone

`newOperation` gains `case "colorization": return { family, codename: option.codename, precision: option.precision }`.
The default model needs nothing: `modelChoice` falls back to the catalogue's first variant, Delhi, which is also the
reference's `defaultModel`.

### D6. `RUNNABLE` and its companions are deleted; the check moves onto `ENHANCEMENTS`

Decided by the user while exploring this change. With colorization on the wire, `RUNNABLE` names all seven families
and filters nothing out; it would stay only as a second list that has to be kept in step with the first.

- `ENHANCEMENTS` is declared `[...] as const satisfies readonly Enhancement[]`. `satisfies` keeps every entry checked
  against `Enhancement`; `as const` keeps each `family` as its literal, so the list's own type says which families it
  holds.
- The compile check moves onto it: `Exclude<Operation["family"], (typeof ENHANCEMENTS)[number]["family"]>` must be
  `never`, so a family added to the wire and not to the list is a type error naming itself - the guarantee `Unoffered`
  gave, now checked against the list the menu actually draws. The reverse direction needs no check: an entry whose
  family the wire cannot carry is a type error at `newOperation`'s call in `AddEnhancement`.
- `RUNNABLE`, `Unoffered`, `_EveryRunnableFamilyIsOffered` and `RUNNABLE_ENHANCEMENTS` are deleted. `AddEnhancement`
  maps `ENHANCEMENTS` directly, and its comments about the filter and the screen-10 divergence are removed.
- `Enhancement.family` stays typed `Family`. The literal types come from `as const`, and the entries' families are
  already all enhancement families because `detection` is left out of the list, which `ENHANCEMENTS`' doc already
  explains.

**Alternative considered: a unit test** that the menu lists the seven in order. It catches the same omission, but at
test time rather than where the union is edited. It is the fallback if `as const` does not type cleanly against the
list's readers (`stores/settings.ts`, `stores/enhancements.ts`, `EnhancementsPage`, `EnhancementList`,
`progressLabelKey`); all of them read `family`, `find`, `findIndex` or `map`, which accept the narrower literal types,
so none is expected to break. If the fallback is taken, it is recorded against task 2.2.

### D7. Where the work sits

| Crate | What |
|---|---|
| `opai` | Nothing. Everything this change calls is published and tested. |
| `gui` (Rust) | The `Colorization` arm of `Requested`, its tests, the refused-family test deleted, the docs. |
| `gui` (frontend) | The union member, `ENHANCEMENTS`' typing and check, `RUNNABLE` deleted, `newOperation`, `ColorizationOptions`, the row's two switches, `AddEnhancement`, tests. |
| `cli` | Nothing. The CLI already reaches colorization through `opai`. |

`rust-sak`: no feature used, and no gap - the change contains no infrastructure. No crate or package is added.

### Sessions, providers and platforms

No session, memory budget or provider code changes. A colorization is opened, cached and released by the same
`Opai::execute` path every other family is, under the three variants' existing `EpProfile`s (CoreML restricted to the
CPU and GPU at FP16, provider defaults at FP32). The change is validated on macOS (CPU and CoreML through the existing
profiles, via the GUI run). Windows and Linux are untested here; nothing in the change is platform-specific.

## Slices

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-colorization` | The `colorization` wire arm; Colorization in the add menu between Face Recovery and Light Adjustment with the default model; the `Delhi, HD` info line; `ColorizationOptions` holding the model tray alone; `RUNNABLE` removed with its check moved onto `ENHANCEMENTS`; the refused-family test and scenario removed | - | **current** |

**This is a new series**, named after the feature. `add-gui-sharpen` was a series of one and is closed.

## Risks / Trade-offs

- **[The graphs are large]** → each is 0.44 to 0.91 GB, so the first colorization downloads for a while under the
  existing download report. Nothing new; Moscow and Novgorod are of the same order.
- **[Colorization on a colour photograph discards its colour]** → the reference behaves the same, and the model
  descriptions frame the family as being for black-and-white photographs. A warning was considered and declined
  (proposal - Not in this change).
- **[`as const` widens the diff in `lib/enhancements.ts`]** → it is one line on the declaration; the entries are
  unchanged. D6 names the fallback.
- **[With the refused-family test gone, a family added to `Family` but not to `Requested` is not caught in Rust]** →
  the frontend check in D6 stops it where the operation is built, and the family could not be sent without a union
  member.

## Migration Plan

Nothing to migrate. `Requested` gains an arm, which is additive on the wire; no stored preference changes shape, and
the default-model preference for colorization already exists in the Settings dialog.
