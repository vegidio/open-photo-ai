## Slices

The series that takes the application from a sidebar whose Add enhancement button is greyed out to
one that runs an upscale and draws the result. It is its own series; the `add-gui-images-1-ipc` ..
`add-gui-images-4-zoom` series closed on the zoom slice and is not continued here.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-enhance-1-seam` | The structured operation on the wire; the single-run slot; the run and stop commands; the progress event and its throttle; the enhanced result served over the existing `opai://` scheme; `ipc/enhance.ts`. No screen | - | **DONE** (archived 2026-09-19) |
| 2 | `add-gui-enhance-2-sidebar` | The per-file enhancement store; the add menu; the sidebar rows with their info line and hover-remove (screens 05 and 14); screen 11 (the model tray and the scale control); the run-and-cancel effect; screen 06 (the progress chip); the enhanced pane drawing the result | 1 | **CURRENT** |

**The series closes here.** Slice 1 estimated slice 2 at fifteen tasks and recorded a fallback -
split the canvas half (the run effect, the chip, the enhanced pane) into a third slice if it
overflowed. It has not: the count is fourteen, because two decisions taken since removed work rather
than added it. The add menu offers one family instead of seven, so there is no per-family disabling to
build; and the report gains a field rather than the window gaining a way to reconstruct one. The
fallback is therefore not taken, and no third slice is opened.

**What this slice leaves for later changes, each its own:** the six other families (each arrives with
its own wire variant, its own options panel, and for face recovery a detection run the window has no
wiring for); autopilot actually suggesting anything; the export queue; the navbar's Dimensions block
switching to the output size once a scale is in play; and the provider-fallback notice that slice 1
answered and ignored.

**Crates.** `gui` alone, and almost all of it the React frontend. `opai` is not edited. `cli` is
untouched. The one Rust change is a field on `RunProgress` in `gui/src/enhance.rs` - D7 - which is the
shape of a Tauri event and therefore `gui`'s by the same test every wire type in that crate passes.
Nothing here is logic the terminal front end could want: a stack of enhancements bound to the
photograph on screen is a property of a window, and the export queue - the one other consumer of these
operations - runs its own chains at its own depth and is a change of its own.

**rust-sak: nothing used directly, and nothing missing.** No download, hash, extraction, temp file,
codec, cache or hardware probe is written here. Everything the run touches reaches `fetch`, `crypto`,
`memo` and `image` through `opai`, by way of the commands slice 1 built. There is no gap in either
direction.

**New crates: none. New npm packages: none** - `radix-ui` and `lucide-react` already carry the
popover and every icon the menu and the rows draw.

**Platforms.** Written for all three, validated on macOS. Two things here are platform-shaped and are
named in Risks: the hover-revealed remove control, which needs a second route on a machine with no
pointer, and the webview caching of an enhanced identity, which slice 1 already recorded as a
Windows-only concern.

**Execution providers.** The processor is whatever the settings store holds; this slice selects,
builds and falls back on nothing, and only re-runs when the user changes it. Validated on CoreML and
CPU, which is what the development machine offers. CUDA and TensorRT are untested by this slice and
untouched by it.

## Context

Everything this slice needs on the far side of the boundary exists and is described in
`add-gui-enhance-1-seam`'s design; see proposal.md - Why for what is missing on this side. What shapes
the design is a short list of facts about the pieces it has to join:

```
  stores/enhancements.ts      lib/enhancements.ts        ipc/enhance.ts        features/preview
  (autopilot only)            ENHANCEMENTS, modelChoice   enhance/cancel        Pane x2, one transform
        |                            |                        |                        |
        +---- Map<path, Operation[]> +--- build an Operation --+--- run, stop, report --+--> enhanced pane
```

- **The store is one boolean today**, and its own comment records that it is the Wails store's
  persisted surface exactly: four fields, of which `partialize` keeps only `autopilot`. The Maps this
  slice adds are the three it left out, and they must stay out of what is persisted.
- **`lib/enhancements.ts` already holds the seven families in pipeline order** and already builds a
  family's model list from the catalogue, including the positional tier rule and the repair of a
  stored selection that no longer names a published model. The settings dialog's default-model rows
  are built from it. It has no icon and no short name yet, and those are the two things a row, a menu
  entry and a progress chip need that it does not publish.
- **`ipc/enhance.ts` is complete and typed**, mints the run identifier before the invoke, and
  subscribes to a global event whose every report names its run.
- **`PreviewImage` draws two panes off one `ImageRecord` and one transform.** Both panes point at the
  same URL today, deliberately, and the transform is stored under `file.identity`. An enhanced result
  has a different identity and different dimensions, which is the one place the existing canvas does
  not already fit - D8.
- **The settings store holds `models` and `processor`**, spelled exactly as this slice needs them: a
  model as `<codename>_<precision>`, a processor as the wire's own `Processor`.
- **Every i18n key this slice draws already exists in all thirteen locales** - the enhancement names,
  the model descriptions, `enhancements.info*`, `enhancements.scale.*`, `enhancements.modelSelector`,
  `preview.progress.*` and `errors.enhanceFailed`. The catalogues were ported whole in
  `add-gui-1-foundation`, so this slice adds keys only where it draws something the Wails app did not.

## Goals / Non-Goals

**Goals:**

- Make the stack the single thing that decides what the enhanced pane shows, so that every control
  added later - a second family, autopilot, a crop - drives the preview by writing to it and by
  nothing else.
- Keep the vocabulary in one place: what a chooser offers and what the wire carries come from the
  catalogue, and this slice adds no second copy of a codename, a precision or a range.
- Leave the menu able to grow into the remaining six families without being rewritten, and make a
  family added to the wire but not to the menu a compile error rather than an omission.

**Non-Goals:**

- **No debounce, and no queue.** A run superseding a run is what the seam is for - D9.
- **No second store for the run.** The backend holds the run slot; a store here would be a second copy
  of state that is already authoritative somewhere else - D6.
- **No new error screen.** A failed run raises the notice this application already has, and the view
  keeps showing what it was showing.

## Decisions

### D1. The stack is `Map<path, Operation[]>` on the existing store, holding the wire's own type

The store gains one Map, keyed by the file's path for the reason the Wails store's own comment gives:
a Map keyed on the record uses reference identity, and any path that rebuilds a record for the same
image - a re-drop, a re-describe - would orphan that file's state and leave the old entry behind
forever.

**It holds `ipc/enhance.ts`'s `Operation` and not a type of its own.** That type is already exactly
what an enhancement is: a family, a model, a precision and the family's own values. A parallel
domain type would be a second spelling of the same four fields, and the conversion between them a
place for them to disagree - which is the duplication D1 of slice 1 removed from the wire and would
be a poor thing to reintroduce one layer up.

**One entry per family, and that is what a row is keyed on.** The wire operation carries no
identifier, and the menu's rule - an enhancement already in the stack cannot be added again - makes
the family unique within the list. The reference keys its rows on an operation id and its own comment
explains what that id is for there: a stale id rehydrated from a previous version's persisted state.
Nothing here is persisted, so nothing stale can arrive, and the family is both stable and unique.

**Three writers, all one level deep:** add (appended, then ordered by the pipeline order
`ENHANCEMENTS` already publishes), replace (the entry of that family), remove (filter). Closing a
file deletes its key; closing all clears the Map.

**Still no `immer`**, which is what slice 1's D6 settled and asked this slice to record rather than
re-litigate: `set(state => ({ enhancements: new Map(state.enhancements).set(path, next) }))` is the
whole of it. The parked comment in `stores/enhancements.ts` is rewritten to say the Maps landed and
still did not need it.

**Not persisted.** `partialize` already keeps `autopilot` alone, and its comment already says the
Maps must not be added to it. This slice is the one that makes that true rather than hypothetical.

### D2. The menu offers what the wire can carry, and the compiler keeps the two in step

The menu is built from `ENHANCEMENTS` filtered to the families `ipc/enhance.ts`'s `Operation` union
carries - one today - with an exhaustiveness check in the same shape `stores/settings.ts` already uses
for its snapshot keys:

```ts
const RUNNABLE: readonly Operation["family"][] = ["upscale"];

type Unoffered<T extends never = Exclude<Operation["family"], (typeof RUNNABLE)[number]>> = T;
export type _EveryRunnableFamilyIsOffered = Unoffered;
```

A family added to the wire by a later slice and forgotten here stops the typecheck, naming itself in
the error. That is the property that makes "the menu grows with the wire" a fact rather than an
intention.

**Why the six are absent rather than disabled.** The design's screen 10 draws all seven and dims what
is already in the stack, and the reference does the same; the dimming is for the case it names. Six
entries that can never be chosen say the application has six enhancements it is refusing to run,
which is a different statement and a false one. This is the deliberate divergence the proposal
declares, taken on the user's decision.

**The already-in-the-stack rule is still built**, because it is the menu's own rule and applies to one
family exactly as it applies to seven: adding Upscale twice would put two upscales in a chain.

### D3. What a new enhancement arrives carrying

The model is the user's stored default for that family, read through `modelChoice(entry, stored)` -
the same call the settings rows make, so the default, the repair of a selection that no longer names a
published model, and the positional HD/SD tier rule are the ones already shipped rather than a second
implementation. `modelChoice` answers a `value` of `<codename>_<precision>`; the wire wants the two
apart.

**Splitting it uses the option's own `codename` rather than a separator.** `ModelOption` carries
`codename` beside `value` precisely so a call site need not split the string, and the settings rows'
comment records why: splitting on `_` works today and stops working the first time a codename
contains one. The precision is what follows that codename.

**An upscale arrives at a scale suited to the photograph** - 4x at or below 1 MP, 2x at or below 4 MP,
1x above - which is the reference's `defaultUpscaleScale` rule and its thresholds, taken as parity.
It reads the record's dimensions, which are already on `ImageRecord` and may be absent for a file
whose header could not be parsed; absent means the largest bucket, 1x, which is the conservative end.

### D4. The scale's range is the catalogue's, per model

The catalogue publishes each variant's parameters, and an upscale's scale is a `range` entry carrying
`min` and `max` - the same constants `Scale::new` enforces, which is the property `ParameterKind`'s own
documentation claims for it: *"a slider built from this cannot offer a value that would be refused"*.
The control reads its bounds from the selected model's own entry rather than from a constant here.

**Per model, not per family**, for the reason `VariantEntry.parameters` is per model: two models of a
family may disagree, and a control built from a family-wide list would offer one of them a value its
sibling refuses. Today every upscale model publishes the same range; the control does not depend on
that staying true.

The typed field follows the reference's own input rule - a trailing decimal separator is held rather
than parsed, so a user can type `1.5` - and a value outside the range is brought inside it rather than
refused, which the seam does again on the far side (slice 1's D7) and the spec states as behaviour.

### D5. The options open in a popover anchored to their row, and the model tray styles off `aria-checked`

The panel is a `shadcn` popover over the `radix-ui` package already installed. Two things about it are
known in advance rather than discovered:

- **`shadcn add` does not leave usable output here.** It rewrites the `cn` import, adds a `cn`
  package that does not exist, writes function declarations instead of arrow functions, imports the
  `React` default, and hard-wires a portal target. Every generated primitive in this project needed
  the same fixes; the task that adds this one includes them.
- **A tooltip wrapped `asChild` around a Radix item clobbers `data-state`.** The model tray is a
  toggle group whose items carry the info marker's tooltip, which is exactly the arrangement
  `DrawerHeader.tsx` hit with the preview-mode group: the item renders with nothing looking selected.
  The tray therefore paints the selection off `aria-checked`, which Radix sets and the tooltip cannot
  overwrite, and a test asserts it the way `Drawer.test.tsx` already does for its own group.

**The anchor is measured, not guessed.** The design puts the panel's top-right corner at the row's
vertical centre, which is `side="left"` with an `alignOffset` of half a row. Radix clamps
`alignOffset` against collision padding, measures `sideOffset` from the trigger rather than from the
popover, and ignores a clipping box that is not the viewport - so the numbers are settled by making
the running window report its own rectangles rather than by reading them off the mockup.

### D6. The run lives in one hook, called once, and holds no store

A single hook owns the whole run: it reads the settled file, the current stack and the processor,
starts a run when any of the three changes, cancels in its cleanup, subscribes once to the progress
event, and answers what the canvas needs - the enhanced result's identity and dimensions, and the
current report.

```
  useEnhancementRun(file)
      |                                   cleanup: cancelEnhance(run)
      +-- enhance(identity, ops, proc) ------------------------------+
      |        |                                                     |
      |        +-- done -> { outcome: "enhanced", identity, w, h } --+--> enhanced pane
      |        |           { outcome: "stopped" }  -> nothing
      |        +-- rejects -> notice (errors.enhanceFailed)
      |
      +-- onEnhanceProgress -> report.run === mine ? keep : discard --> chip
```

**No store**, because the authoritative single-run slot is in Rust and a store here would be a second
copy of it that can disagree. Nothing outside the canvas reads the run: the sidebar rows describe what
*will* be run, not what is running. It is called once, from `PreviewImage`, which is the component
that draws both panes and the chip and is mounted for as long as a file is open.

**The stack is a stable dependency** because the store replaces the array only when it changes, so the
effect keys on the array itself; an absent entry answers with one shared empty array rather than a
fresh one per render, which would restart the run on every render.

**A stopped outcome writes nothing.** Slice 1's D2 made a displaced run answer `stopped` whatever it
returned, precisely so this effect can branch on the outcome and never ask whether the run it is
holding is still the current one. A rejection raises the notice; a stop does not, which is the
difference the seam already reports.

**An empty stack asks for nothing.** The seam accepts an empty chain and answers with the source, but
a run to learn what is already known is a round trip for an answer the window holds; the hook answers
with the source's own identity directly and still stops whatever was in flight.

### D7. The report names the family, so the chip can name it in the user's language

`RunProgress` gains `family`, taken from `opai::Subject::family()` and serialized the way `Family`
already serializes - the same spelling `ipc/catalogue.ts` publishes.

The chip needs a short name in the interface's own language: the design's own screen shows `Upscale`,
and several languages need a shorter word than the full name, which is why the reference keeps a
separate `shortNameKey` per enhancement and why this slice adds one to each entry of `ENHANCEMENTS`.
The report's existing `operation` field is the library's composed English sentence, `Kyoto 4x (FP16)` -
right for a log, untranslatable by a front end.

**Alternatives.** Parsing the family out of the composed name would be reading a sentence the library
is free to reword. Reconstructing it from the stack the window sent would work only while a chain holds
one operation of each family and would silently mislabel the first chain that does not. Sending the
whole `Operation` back in the report would put the wire's own type on an event that exists to be
thinned to a hundred messages.

### D8. The enhanced pane draws the result under the source's transform

`Pane` derives three things from the record it is handed: the URL, the dimensions it fits, and the
identity its transform is stored under. The enhanced result differs in the first two and must not
differ in the third.

```
  original pane :  url = source identity      fit = source w/h      transform key = source identity
  enhanced pane :  url = result identity      fit = result w/h      transform key = source identity
                                                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

Sharing the key is what keeps the comparison a comparison: two panes at different zooms showing
different parts of a photograph is the one thing the second pane is not for. The result has the same
aspect ratio as its source - every operation this application runs preserves it - so both panes fit to
the same box and resolve one transform identically, which is the property `PreviewImage`'s own comment
already relies on. `Pane` therefore takes the transform key as its own input, defaulting to the
record's identity, rather than inferring it.

**Fitting to the result's own dimensions rather than the source's** is what makes the zoom mean what
it says: a 2x result is twice the pixels, so drawing it fitted to the source's box and magnifying
would show a canvas-sized rendition enlarged rather than the real pixels the run produced. The answer
to the run carries those dimensions, which is why it carries them at all.

**Before a run lands, and after one is stopped, the enhanced pane draws the source** - which is what
both panes already do today, so there is no third state to design.

### D9. Every change re-runs, with no debounce

Writing a model or a scale re-runs immediately, and typing in the scale field writes on every
keystroke. That is the reference's behaviour, and the seam was built for it: the backend cancels what
it displaces, and the run cache answers a chain it has already run without touching a model.

**Why not debounce.** A delay would have to be chosen for the slowest case and would then be dead time
in the fastest one, which is the cached path - a case that did not exist in the reference and is
common here. The cost of not debouncing is a cancelled run per keystroke, and cancellation is checked
between tiles, so what is actually spent is at most one tile's work each. Recorded as a trade-off
rather than a certainty: if a drag across the scale field proves to cost more than that in practice,
the place to fix it is here, and the measurement should come first.

### D10. Sessions, memory and lifetimes

Nothing in this slice owns a session, a model or a token. The run slot holds the cancellation token
and the one resident result; the window holds an identity and two integers. Asking for a run is what
both cancels the previous one and drops its pixels, and slice 1's D8 records why that is safe rather
than something to arrange: cancelling makes a run return, and the run returning drops the last handle
on every model it held.

What this slice adds to that picture is one thing: the window can now ask for runs as fast as a user
can type, and each of those asks drops a full-resolution result. The bound is still one, by D2 of
slice 1, and the drop happens at the moment the successor is asked for rather than when it completes.

## Risks / Trade-offs

**A keystroke in the scale field starts a run** → Bounded by D9: the previous run is cancelled as the
next is asked for, cancellation is checked between tiles, and a repeat is answered from the run cache.
Named as the thing to measure first if the scale control feels heavy.

**The hover-revealed remove control has no pointer on a touch machine** → The control is revealed by
focus as well as hover, and is in the tab order, which makes it reachable by keyboard on every
platform. Untested on a touch-only device; this application ships to desktops, so that is a gap rather
than a hole.

**A family added to the wire and forgotten in the menu** → A typecheck failure by D2, naming the
family. The reverse - a family in the menu the wire cannot carry - cannot be written, since the
menu's list is typed as the wire's own family union.

**The chip's label depends on a family the interface has a name for** → A family the catalogue
publishes and `ENHANCEMENTS` does not name falls back to the generic `preview.progress.enhancing`,
which the locales already carry. That is the same class of drift `lib/enhancements.ts` already handles
for a family the catalogue stops publishing, and the same answer: draw something honest rather than
throw.

**The enhanced pane draws pixels the application may stop holding** → A result the run slot has
dropped is refused by the scheme, and the `<img>` draws as broken. It can only happen when a newer run
has displaced it, which is also the moment the window points the pane at something else - so the
window has to be pointing at a stale identity for it to be visible. The webview cache makes it less
likely still, and slice 1 recorded the one platform where that cache exists.

**The popover's anchor is a measurement, not a literal** → D5. The risk is spending the measurement
budget twice, once here and once in the slice that opens the second family's panel; the popover frame
is written once and shared by all seven, so the second panel inherits the number rather than
rediscovering it.

**Fourteen tasks is not much slack** → One of them is the checks the rules require and one is the
locale catalogues, so the slice is twelve tasks of work against a limit of fifteen. If a task proves
to be two - the options panel's three are the likeliest - the split is the same
one slice 1 recorded: the run effect, the chip and the enhanced pane are a coherent piece and could be
a third slice. It is a fallback and not the plan.
