## Context

See proposal.md - Why. The state this design builds on:

- **`opai` is finished for this family.** `VariantEntry::published(Family::Denoise, codename)?.build(precision,
  &values)` resolves Stockholm, Gothenburg or Malmö (`build.rs`), `ParameterValues::with_strength` carries the amount,
  and the catalogue publishes one parameter for the family, `strength`, as `Range { min: 0.0, max: 3.0 }`, per variant,
  with Stockholm listed first. What each strength does to a photograph is the `denoise` capability's contract.
- **The wire has four arms.** `Requested` in `crates/gui/src/enhance/operation.rs` is tagged by family; its two bias
  arms clamp and log. `ipc/enhance.ts`'s `Operation` union mirrors it, and `lib/enhancements.ts`'s `Unoffered` check
  fails the build if `RUNNABLE` omits a member of that union.
- **`IntensityControl` already expects this caller.** Its doc names "a later denoise's 0..300 about 100" as the same
  control handed different numbers, and `markLeft` places a mark at any value inside any range.
- **`LightAdjustmentOptions` and `ColorBalanceOptions` are copies of each other**, differing only in their doc
  comments and the operation type they accept. `ColorBalanceOptions` carries a comment calling the copy deliberate and
  naming a third family as the moment to revisit it (`add-gui-color-balance` D3).
- **The title and the model list are already shared.** `EnhancementRow` wraps every panel in
  `<OptionsPanel title={t(enhancement.nameKey)}>`, and `ModelTray` lists whatever the family's catalogue entry
  publishes. Neither is a panel's concern.
- **Every locale already carries** `enhancements.denoise.name`, `preview.progress.denoise` and `enhancements.info`.
  None carries a word for "Strength". `ENHANCEMENTS` already lists denoise first, with its `Waves` icon.
- **The reference** (`OptionsDenoise`) is `ModelSelector`, `Divider` and `IntensitySelector` with `min={0}`,
  `max={300}`, one mark at 100, and no label - so it shows "Intensity". Its `ENHANCEMENTS.dn` has `defaultAmount: 1`
  and `defaultModel: 'stockholm'`.
- **Screen 12 of the design** draws exactly this panel: `ENH.dn` is `paramLabel: 'Strength', min: 0, max: 300`, and the
  template draws the mark "100" at a third of the track when `min === 0`. The screen's note text describes the shared
  control as the bias families use it ("-100 to 100 ... a 0 mark"); `ENH.dn` is the denoise-specific statement and
  agrees with the reference, so it is the one followed. The screen's value of 50 comes from its `fullOps` fixture
  (`{ type: 'dn', model: 'gothenburg', intensity: 50 }`), not from a default.

## Goals / Non-Goals

**Goals:**

- Denoise runnable end to end from the window, its panel looking as the two bias panels do.
- One implementation of "a model and one amount" behind all three panels.

**Non-Goals:**

- Sharpen on the wire or in the menu (see proposal.md - Not in this change).
- Folding the Rust wire arms together. `add-gui-color-balance` D1 keeps them written out, and it still holds: serde
  needs one variant per tag, and the arm is where a reader looks for a family.
- Any change to `IntensityControl`'s behaviour, to `opai`, to sessions or to execution providers.

## Decisions

### D1. The wire carries the library's unit strength, clamped and logged

`Requested::Denoise { codename, precision, strength: f64 }`, resolved as
`ParameterValues::new().with_strength(Strength::clamped(strength))` with a `tracing::warn!` when `strength` falls
outside `Strength::MIN..=Strength::MAX` - the bias arms' shape, for their reasons: the wire speaks the library's
vocabulary, and a bounded control makes an out-of-range value a fault, better answered with a picture and a log line
than a failed run. Written out as its own arm, per `add-gui-color-balance` D1.

### D2. `IntensityOptions`: one panel, three thin wrappers

`features/enhancements/IntensityOptions.tsx` holds everything the two bias panels copy today: the `ModelTray`, the
`Separator`, the `IntensityControl`, the lookup of the selected variant's range parameter, the ×100 conversion both
ways, and the rule that a chosen model keeps the amount.

Its props are the family's choices plus the pieces of the operation it reads, and it writes back through two
callbacks rather than building the operation itself:

```
enhancement, entry        what the tray draws from
codename, precision       the selected model (modelValue needs only these)
parameter                 "bias" | "strength" - the library's `NAME`, which is how the range is found
label                     already translated
mark                      in percent: 0 for the bias families, 100 for denoise
amount                    in the wire's unit value
onModelChange(option)     the wrapper spreads codename/precision into its own operation
onAmountChange(unit)      the wrapper writes its own field (`bias` or `strength`)
```

**Callbacks rather than a generic over the operation.** A component generic over `Extract<Operation, ...>` with a
`field` key could write the operation itself, but TypeScript cannot check `{ ...operation, [field]: value }` against a
member of a discriminated union without a cast. The wrapper already holds the narrowed type, so a one-line spread there
stays fully typed. That one line - which field this family's amount lives in - is the one thing the shared panel
cannot know.

**The range is not a prop.** It is the selected variant's published parameter named `parameter`, times 100, exactly
as the bias panels read it today. Passing `min`/`max` would restate numbers `opai` publishes, and a wrapper hard-coding
0..300 would go on offering it if the library's range moved. Found on the **selected** variant, because
`VariantEntry.parameters` is per model.

**The wrappers stay** - `LightAdjustmentOptions`, `ColorBalanceOptions`, `DenoiseOptions` - with the props
`EnhancementRow` already passes (`enhancement`, `operation`, `entry`, `onChange`), so the row's switch gains one case
and changes no other. Each is a few lines: its label key, its mark, its parameter name, and its field.

**This reverses `add-gui-color-balance` D3.** That decision kept a panel per family so each could change without the
other, and named a third family as the point to reconsider. The per-family entry point survives as the wrapper, so a
reader looking for "the denoise panel" still finds a file of that name; what is no longer per family is the logic,
which was identical. The "deliberate copy" comment in `ColorBalanceOptions` is removed, since the copy no longer
exists.

### D3. The label is "Strength", under a new key

`DenoiseOptions` passes `t("enhancements.strength")`, as screen 12 names it, and as the rule `add-gui-color-balance`
D2 set - the label follows the parameter type - gives for `opai::Strength`. The reference showed "Intensity" only
because it passed no label.

The key is added to all thirteen locales beside `enhancements.intensity` and `enhancements.bias`. Each translation
SHALL differ from that locale's `enhancements.intensity`, since the two words sit side by side in the same key block
and a later Sharpen may need to choose between them.

### D4. A new denoise arrives at 1.0, with the catalogue's first model

`newOperation`'s `denoise` arm returns `{ family, codename, precision, strength: DEFAULT_STRENGTH }`, a new constant
of `1` beside `DEFAULT_BIAS`: the reference's `defaultAmount`, and the value the slider marks. At 1 the result is the
model's output unchanged (`denoise` spec), which is what someone adding a denoise is asking for; screen 12's 50 is
fixture data (see Context).

The default model needs nothing: `modelChoice` falls back to the catalogue's first variant, which is Stockholm - also
the reference's `defaultModel`.

### D5. First in the menu

`denoise` is added to the front of `RUNNABLE`. The menu draws `ENHANCEMENTS` filtered to `RUNNABLE`, and
`ENHANCEMENTS` already puts denoise first, so the order is right without further work; the list's doc (family count,
order note) is updated to match.

### D6. The row's info line

`EnhancementRow`'s info switch gains a `denoise` case reading `enhancements.info` with `Math.round(strength * 100)`,
beside the bias cases. It is a case of its own rather than a shared fall-through, because the field it reads differs.

### D7. Where the work sits

| Crate | What |
|---|---|
| `opai` | Nothing. Everything this change calls is published and tested. |
| `gui` (Rust) | The `Denoise` arm of `Requested` and its tests. |
| `gui` (frontend) | The union member, `RUNNABLE`, `DEFAULT_STRENGTH`, `newOperation`, `IntensityOptions`, the three wrappers, the row's two switches, one i18n key, tests. |
| `cli` | Nothing. The CLI already reaches denoise through `opai`. |

`rust-sak`: no feature used, and no gap - the change contains no infrastructure. No crate or package is added.

### Sessions, providers and platforms

No session, memory budget or provider code changes. A denoise is opened, cached and released by the same
`Opai::execute` path every other family is, under the three variants' existing `EpProfile`s, and Stockholm's
divergence guard runs inside `opai`. The change is validated on macOS (CPU and CoreML through the existing profiles,
via the GUI run). Windows and Linux are untested here; nothing in the change is platform-specific.

## Slices

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-denoise` | The `denoise` wire arm; Denoise first in the add menu at a strength of 100%; the `Stockholm, 100%, HD` info line; `IntensityOptions` shared by the Light Adjustment, Color Balance and Denoise panels; the Strength control over 0..300% marked at 100 | - | **current** |

**This is a new series**, named after the feature. `add-gui-color-balance` was a series of one and is closed.

## Risks / Trade-offs

- **[The refactor changes the bias panels by accident]** → their existing test suites are kept unchanged and must pass
  against the wrappers; they cover the label, typing, clamping, a slider commit, a model switch keeping the amount and
  opening without writing. A launch check covers how they look.
- **[Typing `150` writes `1`, then `15`, then `150`]** → three runs, each stopping the one before - inherited from
  `IntensityControl`, accepted there. A denoise run is slower than a bias run, so this is more noticeable here, but a
  debounce would change every family's control and is not this change's decision.
- **[A minus sign is held in a control whose range starts at 0]** → harmless: `-` writes nothing, and any number typed
  after it is clamped to 0. Removing the hold per family would put a family decision back into the shared control.
- **[The label departs from the reference]** → one key; `enhancements.intensity` exists in every locale if it is ever
  wanted instead.

## Migration Plan

Nothing to migrate. `Requested` gains an arm, which is additive on the wire; no stored preference changes shape, and
the default-model preference for denoise already exists in the Settings dialog.
