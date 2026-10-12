## Why

`add-gui-enhance-1-seam` built the whole seam - the structured operation on the wire, the single-run
slot, the run and stop commands, the throttled progress event, the enhanced result served over
`opai://` - and nothing calls any of it. The Add enhancement button is still disabled, which is what
`Sidebar.tsx`'s own note asks for until this slice.

This is the slice that gives the seam a screen. It is the second and last of the series: after it, a
user can add an upscale to the photograph they are looking at, choose the model and the amount,
watch it run, and see the result drawn beside the original.

## What Changes

- **Each image carries its own enhancement stack.** The store gains the per-file Map its Wails
  counterpart has, keyed by path for the reason that store records - a Map keyed on the file object
  uses reference identity, and any path that rebuilds a record for the same image would orphan its
  state. Closing an image discards its stack; nothing about it is persisted, which is what
  `partialize` already says.
- **Add enhancement goes live**, and opens a menu of what this application can run. Today that is
  Upscale alone.
- **The stack is drawn as a row per enhancement**: its icon, its name, and a line beneath naming the
  model, the amount and the quality tier - `Kyoto, 2x, HD`. Hovering a row reveals the control that
  removes it; pressing the row itself opens its options.
- **Upscale's options are a model tray and a scale control.** The tray offers every upscale model at
  every precision the catalogue publishes it at, which is the same list the settings dialog's default
  chooser is built from. The scale is a typed multiplier with a Max shortcut beside a 1x / 2x / 4x /
  Custom group, bounded by the range the catalogue publishes rather than by a constant written here.
- **A new enhancement arrives at the user's default model**, as the settings dialog stored it, and at
  a scale chosen to suit the photograph - 4x for a small image, 2x for a medium one, 1x for a large
  one, which is the reference's own rule.
- **Any change to the stack re-runs the preview**, and stops the run it replaces. Changing image or
  processor does the same. An empty stack draws the source.
- **The canvas reports the run**: a chip over the preview carrying the enhancement's name and how far
  the whole chain has got, or the download's own percentage while a model is being fetched. It is
  absent when nothing is running, and a run made entirely of results the application already had
  therefore shows nothing at all.
- **The enhanced pane draws the result.** The original pane goes on drawing the source, which is what
  makes the two-pane and split comparisons show a comparison for the first time.
- **A report names its enhancement's family.** The event carries the operation's composed English
  name today (`Kyoto 4x (FP16)`); the chip shows a short name in the language the rest of the window
  is speaking, and composing that from a translated name is not something a front end can do from a
  sentence. One field on the report closes it.
- **A failed run raises a notice; a stopped one does not.** A user who changed their mind has not
  been told their enhancement broke - which the seam already reports distinguishably.

Out of scope, and named so the next slice finds them: autopilot still suggests nothing, the six other
families are not offered, the export button stays unavailable, and the navbar's Dimensions block goes
on reporting the source's size rather than the output's.

## Capabilities

### New Capabilities

None. The capability this slice completes was created by slice 1.

### Modified Capabilities

- `gui-enhance`: gains the interface half - the per-image stack, how an enhancement is added,
  described, adjusted and removed, when a run is asked for and stopped, what the canvas reports while
  one is working, and which pane draws the result. Its one existing requirement to change is *A run
  reports what it is doing while it works*, which gains the family on the report so the interface can
  name the enhancement in the user's own language.

## Impact

**Crates: `gui` only.** `opai` is not edited; `cli` is untouched. Within `gui` the work is almost
entirely the React frontend - a store, a menu, a row, a popover, two controls, an effect, a chip and a
pane. The one Rust edit is a field on `RunProgress` in `enhance.rs`, carrying `opai::Subject::family()`
onto the wire so the window can translate it. That belongs in `gui` for the reason every wire type
there does: it is the shape of a Tauri event, which the terminal front end could not want. Nothing
here is logic a second front end would need - the export queue, which is the one other consumer of
these operations, runs its own chains at its own depth and is a later change.

**rust-sak: nothing used, and nothing missing.** This slice downloads, hashes, extracts, decodes,
caches and probes nothing of its own. Everything it asks for reaches `opai` through the commands slice
1 built, and reaches `fetch`, `crypto`, `memo` and `image` from there.

**New crates: none. New npm packages: none.** The popover the options panel opens in arrives through
`shadcn add` into `components/ui/`, the way every other primitive in this project did, and it is
generated against the single `radix-ui` package this project already depends on. Every icon the menu
and the rows need is in `lucide-react`, which is already the icon library.

**Affected code:** `frontend/stores/enhancements.ts`, `frontend/lib/enhancements.ts`,
`frontend/features/sidebar/Sidebar.tsx`, a new `frontend/features/enhancements/` with the menu, the
row, the options popover, the model tray and the scale control, a new run hook,
`frontend/features/preview/PreviewImage.tsx` for the enhanced pane and the progress chip,
`frontend/ipc/enhance.ts` for the added field, `frontend/i18n/locales/*.json`, and `src/enhance.rs`.

**Tasks: 14**, one under the limit. The design's recorded fallback - splitting the canvas half into a
third slice - is therefore not taken, and the series closes here.

**Deliberate divergences from the reference's behaviour:**

- **The add menu offers Upscale alone, where the reference offers all seven.** The wire carries one
  family, so six of those entries could only be drawn as permanently unavailable - a menu whose shape
  promises six enhancements this application cannot run. Each family arrives in the change that makes
  it runnable, and the menu grows with them. This also diverges from screen 10 of the design, which
  draws all seven and dims what is already in the stack; the mockup describes the finished menu, and
  the dimming it specifies is for the case it names.
- **The progress chip names the enhancement from the report's family**, where the reference parses it
  out of the composed operation id. The id that carried it is what D1 of slice 1 removed; the family
  is the part the interface actually needed, and sending it is cheaper than reconstructing it.
- **Osaka's description is shortened**, in all thirteen catalogues, from the paragraph the reference
  carries. This slice is the first thing in this application to *draw* a model description at all -
  they were ported whole in `add-gui-1-foundation` and read by nothing until the tray's info marker -
  so it is also the first time their length is a constraint rather than a string in a file. The
  reference's version runs to about five hundred characters against about three hundred for its three
  siblings, and it is the only one quoting a download size, a memory figure and the SD variant's own
  numbers: in a 288px hover tooltip that is a wall of text where the other three are a sentence. What
  it says now is what the others say - what the model is for, and that it is slow and wants a good
  GPU. The figures it drops belong to the installer, which reports them at the point the user is
  actually waiting for them.
The model tray's info marker is **not** one of these: the reference puts the description on the first
of each model's precisions and leaves the second bare, which is what the design's screen 11 draws and
what this slice reproduces.
