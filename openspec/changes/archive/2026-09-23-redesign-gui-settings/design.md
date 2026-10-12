## Context

See proposal.md - Why. This redraws an existing dialog, so the draft model and the store stay where they are:
`stores/settings.ts` holds what is in force and changes only when Save calls `apply`, and `features/settings/draft.tsx`
holds the dialog's draft, which is discarded on Cancel. What changes is how the dialog is laid out, one new stored
field, and a reset that writes defaults into the draft.

**What the dialog is today.** `sections.ts` declares four sections and the sixteen setting ids under them. The nav
(`SettingsNav.tsx`) renders the table as an index and `scrollIntoView`s the chosen row. `SettingsDialog.tsx` renders
every section in one scrolling pane, each under a `#1f1f23` band. `rows.tsx` has five row shapes (select, switch,
button, slider, radio), each a label, a control and a description below them. The export slider draws a tick at the
format's starting value.

**The design authority** is screens 16, 16b, 16c and 16d in `Open Photo AI (screens).dc.html` (the `settings2` dialog
with `sv2: general | performance | enhancements | export`). The card is the same 768x640 with the same 48px title bar.
Below it, a 184px nav on `#141417` holds four 34px entries, each an icon and a label. The page pane has 24px padding
and 20px gaps and opens with an 18px title over a 13px `muted-foreground` line. Rows are grouped in `border` cards with
an 8px radius and 14px/16px padding, each row a 14px label over a 12px `foreground-dim` line with the control on the
right. The footer puts Reset to defaults on the left as a quiet text action and keeps Cancel and Save on the right. The
mockup's quality values (50/80/90/80) and its Autopilot state (colorization and color balance off) are sample data,
not defaults.

**Crates: `gui` only**, all of it in the frontend. **rust-sak: nothing used, nothing missing** - there is no Rust in
this change. **New crates or npm dependencies: none**; one generated component, `input`.

**Platforms.** Validated on macOS, where the processor list is Auto, CoreML and CPU. The NVIDIA cards (TensorRT, CUDA)
are exercised in tests from a synthetic provider report and are not seen on real hardware. No execution provider is
exercised: nothing here runs a model.

## Goals / Non-Goals

**Goals:**

- Each preference is on exactly one page, which a type check enforces rather than a reviewer.
- "Default" has one definition, read both by a first launch and by Reset to defaults.
- A family added to `ENHANCEMENTS` gets its Enhancements row, its Autopilot switch and its "allowed" default without a
  second edit.

**Non-Goals:**

- **Running Autopilot or narrowing an analysis.** The preference is stored and nothing reads it yet. The slice that
  wires Autopilot into the GUI passes it to `opai`'s family filter.
- **Changing the sidebar's Autopilot switch.** It remains the on/off switch for Autopilot as a whole, and the
  per-family choice narrows what it considers when it is on.
- **Remembering the last page viewed.** The dialog always opens on General (see the spec).

## Decisions

### 1. A page table replaces the section table, and it names preferences rather than rows

`sections.ts` becomes `pages.ts`: the four pages, each with its id, its lucide icon (`SlidersHorizontal`, `Cpu`,
`Sparkles`, `Upload`, the icons the design's paths draw), its title and description keys, and **the `SettingsData`
keys it holds**:

```
general      -> language, background, analytics
performance  -> processor
enhancements -> models, autopilotExcluded
export       -> quality
```

That last column is what Reset reads (decision 4) and what makes "every preference is on exactly one page" checkable.
A type-level assertion checks that the union of the pages' keys is exactly `keyof SettingsData`, and a test checks
that no key is listed twice. A preference added to the store without a page is then a compile error, not a setting
nobody can reach. That is the same guarantee the old table gave for nav entries and rows, moved to the level the new
layout needs.

The page body is a `Record<PageId, ReactNode>`, total over the ids, as `SECTION_ROWS` is today. The nav becomes a list
of four buttons with `aria-current="page"` on the active one. The active page is `useState` in `SettingsDialog`,
initialised to `general`. The draft already outlives a page switch, because it lives in `useSettingsDialog` rather
than in the page components, so moving between pages cannot lose a change.

*Alternative:* keep `sections.ts` and render one section at a time. Rejected because every entry-level id, and the
anchor-and-scroll machinery built on them, would be kept for nothing.

### 2. The Autopilot preference is stored as the families switched **off**

`SettingsData` gains `autopilotExcluded: Family[]`, which defaults to `[]`.

Storing the exclusions rather than the inclusions is what makes the two defaults in the spec hold without a repair
step. A user who has never chosen has nothing stored, which means every family is allowed. A family added in a later
version is absent from the stored list, so it is allowed too. With an inclusion list, each of those would need a
repair that adds the missing family, and it could not tell a family the user switched off from one the list had never
seen.

The repair on rehydrate follows the store's existing pattern. A value that is not an array reads as `[]`, and entries
that are not an entry of `ENHANCEMENTS` are dropped - junk, a family removed from the library (which goes silently),
and `detection`, which is a `Family` but not an enhancement and has no switch. Duplicates are dropped, and the result
is in `ENHANCEMENTS`' order.
The consumer (not in this change) computes `ENHANCEMENTS` minus the exclusions and hands the result to the analysis as
its set. An empty result is the "suggest nothing" case the library already treats as a valid empty set.

*Alternative:* a `Record<Family, boolean>`. It has the same "a new family defaults to on" problem in a different
form: a missing key has to be read as `true` everywhere the record is read, and the store would be one misread away
from a family that is silently off.

### 3. One definition of each default

`stores/settings.ts` gains `settingsDefaults(): SettingsData`, which returns `detectLanguage()`, `"particles"`,
`true`, `"auto"`, `{}`, `[]` and a copy of `DEFAULT_QUALITY`. The store's initial state is built from it, so a first
launch and a reset cannot disagree about what a default is.

It is a function rather than a constant for two reasons. The language default is detected, and records must not be
shared between two drafts.

`models: {}` is the correct default rather than a shortcut. An empty record already reads, where it is drawn, as each
family's first catalogue model at its first precision, which is the default the spec states. Writing the catalogue's
answer into the store would bring in exactly the dependency `ModelChoices` was made partial to avoid.

### 4. Reset writes the page's defaults into the draft

The footer's Reset to defaults calls `update(pick(settingsDefaults(), PAGES[active].keys))`. It uses the same `update`
every control calls, so a reset is a draft by construction: Cancel discards it and Save applies it, with nothing
special-cased on either path.

On General this includes the language, which is what the user chose ("current page only"). The language still
changes only on Save, when `useSettingsDialog` calls `changeLanguage` as it does today. If the draft already holds the
defaults, the reset is a no-op and the button stays enabled. Disabling it would mean comparing drafts deeply for a
button that is harmless to press.

It is a `Button variant="link"`-style quiet action in `muted-foreground`, placed on the left as the design shows,
rather than a third filled button that would compete with Save.

### 5. Rows become cards of rows

`rows.tsx` is rebuilt around two pieces:

- `SettingsCard`: the bordered, rounded group.
- `SettingsRow`: a label over its description on the left, the control on the right, with a `border-t` between rows
  inside a card. The id and anchor are dropped because nothing scrolls to a row any more.

The select, switch and button shapes stay as controls placed inside a `SettingsRow`, keeping today's fill pins
(`SELECT_FILL`, `BUTTON_FILL`) and their comments. The control width goes from 196px to the design's 180px. The
slider and radio shapes move into the pages that use them, because each now has one caller with a layout of its own.

### 6. General: the background as two live tiles

Each option is a 52px tile over a radio and a label. The whole tile is the hit target, and the `RadioGroup` keeps the
semantics a screen reader announces. The chosen tile gets a `primary` outline, as `RADIO_FILL` already styles the
checked circle.

The particle tile renders the real `Particles` component with the design's sample parameters (40 particles, size 0.4,
white). The dotted tile uses the canvas's dot colour at the design's 16px pitch rather than `DOTTED_SURFACE`'s 48px,
because at 48px a 52px-high tile shows one row of dots and does not read as a grid. The colour comes from
`--color-surface-dot`, so the tile and the canvas cannot disagree about what a dot looks like; only the pitch is
scaled to the tile.

This is a second particle canvas while the dialog is open, 40 particles in a small box. `Particles` already stops when
unmounted, and the tile is mounted only while General is showing.

### 7. Performance: radio cards over the provider report

`offeredProcessors` and `resolveProcessor` are unchanged, so the options are still the report's answer, in
`PROCESSORS`' order. Each offered processor becomes a card containing a radio, the name, and a sentence under it. Auto
carries a `Recommended` badge (the design's accent-tinted pill, `primary` at about 14%). The chosen card gets a
`primary` border and a `primary`/6% fill. It is a single `RadioGroup` with the card as the label, so the whole card is
the hit target.

The sentences are the design's for Auto, TensorRT, CUDA and CPU. **CoreML has no screen**, so its sentence is written
here in the same register and carries forward the existing `description_coreml` caveat: *"Runs the models on the
Apple Neural Engine and GPU. Much faster than the CPU, though not every model can use it."*

### 8. Enhancements: one table, two controls per row

The page is a card containing a 36px header row (Enhancement, Default model, Autopilot) and one 50px row per entry in
`ENHANCEMENTS`, in a `minmax(0,1fr) 180px 72px` grid. Each row has the family's icon and name, the existing
model select (`modelChoice` and `modelLabel`, with separators between models, unchanged), and a `Switch` for the
Autopilot choice. The switch's `aria-label` names the family, for example "Autopilot: Denoise", because a column of
seven identical switches is otherwise unaddressable.

The per-row descriptions are removed, because the table has no room for them. `Enhancement.descriptionKey` in
`lib/enhancements.ts` is removed with them: the dialog was its only reader (`ModelTray` builds its own keys).

### 9. Export: a filled slider and a typed box per row

The four formats share one card. Each row is the format's name in a 72px column, the `Slider` filled to its value in
`primary` (the generated range already fills, so this only needs the track and range colours checked against the
design), and a 56px `Input` in the monospace face.

The box keeps **its own text state** while focused, so typing "4" on the way to "42" does not snap the slider to 4 or
clamp to the minimum mid-keystroke. On blur or Enter it commits. The text is parsed, and then:
- a non-number or empty text reverts to the draft's value;
- anything else is rounded and clamped to `MIN_QUALITY`..`MAX_QUALITY`, then written with `update`.
While the box is not focused it shows the draft's value, so a slider drag and the box never disagree. Escape is left
alone: it cancels the dialog as it does everywhere else on the surface, which discards the uncommitted text along with
the rest of the draft.

`type="text"` with `inputMode="numeric"` rather than `type="number"`: a number input's spinner and its browser-specific
handling of invalid text would fight the clamp-on-commit rule.

The starting-value tick (`data-slot="slider-mark"`) is removed, as the design draws none.

### 10. Copy, in all thirteen locales

Added:
- `settings.pages.{general,performance,enhancements,export}.{title,description}`;
- `settings.performance.processor.{auto,tensorrt,cuda,coreml,cpu}.description` and `settings.performance.recommended`;
- `settings.enhancements.columns.{enhancement,model,autopilot}` and `settings.enhancements.autopilotLabel` (the
  switch's accessible name, with `{{name}}`);
- `settings.export.valueLabel` (the box's accessible name, with `{{format}}`);
- `settings.reset`.

Changed in place to the design's shorter copy: `settings.app.{language,analytics,logs}.description`, and
`settings.app.background.{title,description}` ("Preview background").

Removed, because nothing reads them any more: `settings.sections.*`, `settings.performance.aiProcessor.description*`,
`settings.enhancements.*.description` and `settings.export.*.description`.

The locale parity test catches a key present in one catalogue and not another. The twelve translations are written as
part of the task, not left in English.

### 11. `shadcn add input`

It is added as generated and then given the six hand-edits every `shadcn add` in this crate needs. It also gets the
same `dark:bg-input/30` pin the select needs: the design draws the box as a `background` well, like the select.

## Risks / Trade-offs

- **Longer locales overflow a page.** German and Russian run 30-40% longer than English, and the Enhancements page at
  7x50px plus its header comes within about 30px of the pane. → The page pane keeps `overflow-y-auto` with
  `scrollbar-thin`, so overflow scrolls instead of clipping. The manual check includes the longest locale.
- **The nav can no longer take the user straight to a setting.** For example, "JPEG quality" is two clicks instead of
  one. → Accepted, as the design's choice. Four pages of at most seven rows are short enough to scan.
- **A second particle canvas.** → 40 particles in a 52px tile, mounted only while General is shown. If profiling
  shows a cost, the tile can fall back to a still frame without changing the spec.
- **Removing i18n keys breaks a reader nobody knew about.** → `ParseKeys` typing turns a removed key that is still
  read into a `typecheck` failure, not a raw key on screen.

## Migration Plan

No data migration. `settings-storage` gains `autopilotExcluded`. A stored state without it (every existing user)
reads as `[]`, meaning every family is allowed, which is the stated default. Rolling back to the previous build
ignores the extra field.
