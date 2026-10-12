## Context

See proposal.md - Why. What matters here is that both sides of the boundary are already finished: slice 6 put the
provider report, the log path, the reveal and the catalogue on the wire, so this slice adds no Rust logic at all. What
it adds is a persisted store, a dialog and fifteen rows drawn from three sources - the catalogue, the provider report
and the user's own past choices. The one `.rs` line it does touch is a display label, corrected once the dialog put it
on screen; see proposal.md.

**What already exists in the frontend.** `ipc/setup.ts` exports `SupportedProviders` and the setup store keeps the
resolved report in `providers`, documented as being kept *"because the settings dialog's processor list is built from
it"*. `ipc/catalogue.ts` fetches the catalogue once and keeps it, exposing `FamilyEntry`, `VariantEntry`, `Precision`
and `forgetCatalogue` for tests. `ipc/logs.ts` exports `logPath` and `revealLog`, with the rejection deliberately
propagated: *"a button that quietly does nothing leaves them with no file and no reason."* `components/ui/` holds
dialog, button, switch, slider, separator, checkbox, toggle, toggle-group, progress and tooltip; **select and sonner
are the two it does not have.**

**The catalogues are already complete.** All thirteen locale files were ported whole in slice 1, so every key this
dialog needs already exists in all thirteen: `settings.title`, the four section names, every row's title and
description, `settings.performance.aiProcessor.description_{auto,tensorrt,cuda,coreml,cpu}`, `models.label` with
`models.quality.{hd,md}`, `common.{cancel,save,close}` and `errors.showLogsFailed`. **No i18n key is added by this
slice** - which is unusual enough to be worth stating, because the project's own task rule asks for one and there is
nothing for it to cover.

**The design authority** is screens 16 and 16b: a 768x640 card, a 48px title bar with a close box, a 208px nav column,
sections with a sticky-looking subheader band and rows of `label + control` over a muted description, and a footer
with Cancel and Save. Its notes: *"A nav list on the left (the MUI tree was always fully expanded, so it becomes a
plain list) and scrolling sections on the right. Language and processor changes are drafts until Save"*, and *"One
slider per lossy encoder. The scales are not comparable between formats, so each keeps its own default mark and
remembered value."*

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | done |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs | 1 | done |
| 3 | `add-gui-3-init-plan` | `InitOptions::on_plan` and the already-installed report, so the install list, its per-dependency sizes and each dependency's outcome are observable before and during a first run. `opai` only | - | done |
| 4 | `add-gui-4-setup-progress` | Screen 02 - `prepare_library_path` at the top of `main`, the `initialize` command, progress and plan events, the managed `Opai` handle, the setup dialog, automatic start, Quit | 2, 3 | done |
| 5 | `add-gui-5-setup-error` | Screen 02b - the failure dialog, per-dependency outcome, the failure kind on the wire, Try again and Quit | 4 | done |
| 6 | `add-gui-6-settings-ipc` | The three facts the Settings dialog is drawn from, on the wire: the provider report off `initialize`, the log file's location and revealing it, the model catalogue. No UI | 4 | done |
| 7 | `add-gui-7-settings-dialog` | Screens 16 and 16b - the reusable dialog title bar, the persisted settings store with Cancel/Save, the dialog frame and nav rail, and **all fifteen rows** | 6 | **CURRENT** |

**Slice 7 is this change**, and it is the last of the Settings pair. The remaining 16 designed screens are not yet cut
into slices; they are cut when they are reached, and the table carries only what has been committed to.

**Crates.** `gui`, and within it the frontend, plus one `.rs` display label in `opai` - `Malmö`, see proposal.md. No
`Cargo.toml`, no `tauri.conf.json` and no `capabilities/default.json` is touched. `cli` is untouched.

**rust-sak: nothing used, nothing missing.** Covered in proposal.md - there is no infrastructure in this slice.

**New crates: none. New npm dependencies: one**, `sonner`, plus two generated components (`select`, `sonner`). Decided
below.

**Platforms.** Validated on macOS, the development machine. Nothing here is platform-specific except the reveal, which
is slice 6's command and unchanged. Execution providers: none are exercised - the provider report is read and the
choice is persisted, and no model is built from either.

## Goals / Non-Goals

**Goals:**

- Every one of the fifteen rows is live. Nothing is drawn inert; that is what slice 6 bought.
- The frontend restates no model vocabulary. Model names, labels, precisions and which enhancements have models all
  come from the catalogue.
- Cancel is total. Restoring is driven by one enumerated list of the store's data fields, so a field added later
  without being covered is a compile error rather than a setting Cancel quietly keeps.
- The catalogue's arrival is not something the store has to wait for. A stored model is resolved where it is drawn.

**Non-Goals:**

- **Acting on the chosen processor.** Restated from slice 6's design because it is this slice that draws the control:
  there is no session to unload, so the choice is persisted and the slice that runs a model reads it.
- **Sending analytics.** The switch is shipped and nothing reads it yet - see the decision below.
- **The first-run TensorRT dialog.** Not a settings row, not a designed screen, not in the slices table.
- **Export itself.** These four qualities are preferences with no consumer until the export slice; the Export dialog
  will read this store rather than keeping its own.
- **A scroll-driven navigation highlight.** Decided below.

## Decisions

### The default model comes from the catalogue's own order, and the frontend states no codename

For each family, the model a user gets before they have chosen one is `variants[0]` at `precisions[0]`. There is no
table of seven codenames in the frontend and no `default` field in `opai`.

The reference does state them, and says why: *"Stated rather than implied by list position, because presentation order
and the sensible default are not the same thing: upscale lists Tokyo first but defaults to Kyoto."* That is a real
observation, and taking the order instead is the one **behavioural** divergence in this slice - upscale defaults to
Tokyo. It is taken deliberately, because both alternatives cost more than the row they fix:

- **Seven codenames in the frontend.** This is exactly the duplication `opai::catalogue()` was built to remove, and
  its module doc names the failure: in the reference *"a parameter's bounds are literals in a React component... so a
  control can offer a value the library refuses."* A default written here rots the same way - a model renamed or
  retired in the library leaves a settings row pointing at nothing.
- **A `default` on `FamilyEntry` in `opai`.** The library has already refused to hold this. `autopilot.rs`:
  *"which of a family's models to run is a standing user preference, and this library has nowhere to read one from. An
  arm that carried one would either invent a default the user never chose or force the..."* Adding a default to the
  catalogue is that same invention, one module over.

The catalogue's order is not arbitrary either - it is documented as *"the order a chooser should offer them"*, so the
first entry is already the library's own answer to "what should a user see first". For six of the seven families it is
also the reference's default; for upscale it is Tokyo, which the catalogue lists first because it is the conservative,
general-purpose choice. This is written into proposal.md as the slice's declared parity gap.

*Alternative considered.* **Deriving the default from `Suggestions`.** Rejected: autopilot names a family and never a
variant, which is the quote above.

### The quality tiers are positional, read off each variant's published precisions

"HD" is `precisions[0]`, "SD" is `precisions[1]`, per variant. `models.label` (`"{{model}} {{quality}}"`) composes the
option's text from the catalogue's `label` and `models.quality.{hd,md}`.

The reference keeps a map for this - `{ hd: 'fp32', md: 'fp16' }` with an override for Osaka, whose comment explains
that *"no fp32 build of it was ever published, and its diffusion transformer is now also published quantised to
int8"*. That map exists because the Go bindings publish no order. The Rust catalogue does, and pins it:
`FloatPrecision::PRECISIONS` is `[Fp32, Fp16]` and `OsakaPrecision::PRECISIONS` is `[Fp16, Int8]`, each documented as
*"in the order a chooser should offer them"*, each with a test asserting the widened list matches the contract it
widens. Reading the position therefore reproduces the reference's map exactly, including its one override, from data
the library already guarantees.

It also generalises where the map does not: `OsakaPrecision`'s own doc says a catalogue that decided which pair a
model gets would be *"silently wrong the moment a second diffusion model published a different pair"*. A positional
read is right for that model on the day it lands.

**A variant publishing one precision, or three, is handled rather than assumed away.** One precision means one option
and no tier suffix - a tier label on a choice of one says nothing. Three would mean a third tier with no name in the
catalogues; the code shows the precision's own spelling for anything past the second rather than mislabelling it. No
such model is published today.

### The stored model selection is `<codename>_<precision>`, and it is resolved where it is drawn

The store holds one string per family, in the reference's own spelling (`stockholm_fp32`, `osaka_int8`), and it holds
**nothing** for a family the user has never chosen for.

This is what keeps the catalogue's arrival out of the store. The catalogue is an `invoke`; the persisted store
rehydrates synchronously at import. A store that validated its models on rehydrate would either have to wait for a
promise or validate against nothing. Resolving in the row instead means one function - "given this family's entry and
this stored string, what is selected?" - covers three cases with one rule: absent, present-and-valid, and
present-but-naming a model or precision the library no longer publishes. All three fall back to `variants[0]` at
`precisions[0]`, which is the default requirement and the repair requirement satisfied by the same line.

The other three preference kinds *are* repaired on rehydrate, because each can be judged against something already
present: the language against the shipped catalogues, the quality against fixed bounds, the processor against the
report the setup store already holds by the time a dialog can be opened.

*Alternative considered.* **Normalising models on rehydrate, as the reference does.** Rejected: the reference can,
because its model registry is a module constant. Here it is a promise.

### One enumerated list of data fields drives both the snapshot and the restore

The store keeps its data keys in one `as const satisfies readonly (keyof SettingsStore)[]` list, and both
`saveSnapshot` and `restoreSnapshot` iterate it. Arrays and records are copied rather than referenced on the way into
the snapshot.

Lifted from the reference unchanged, including the reason: *"a new or renamed settings field updates one place and
stays covered, instead of being silently left out of the snapshot until someone notices Cancel doesn't revert it."*
The `satisfies` is what makes it a compile error rather than a habit. The copy is what stops a later edit from
mutating the snapshot and turning Cancel into a no-op.

The snapshot is taken when the dialog mounts and lives inside the store's closure rather than in React state: it must
survive every re-render the dialog does while it is open, and it is not something anything renders.

### Storage is closed for writing while the dialog is open, so only Save records a preference

The rows write the store as the user touches them, and `zustand/persist` writes storage after every `set` - so on its
own, the Cancel/Save mechanism covers the store and not the file behind it. The persisted storage is therefore wrapped
in one whose `setItem` drops the write while a draft is open. Reads are never gated: rehydration happens at import,
before any dialog exists.

Without this, a draft escapes by the one route no button is on. Picking a language and then quitting - or losing the
process - before Cancel or Save leaves the choice on disk, and the next launch reads it as a preference the user
chose. That is a cancellation the user can never perform, which is what makes it worse than the Cancel bug the
snapshot exists to prevent: Cancel at least runs.

**The gate is the snapshot itself, not a second flag.** A draft is a period rather than a value, and that period is
exactly the life of the recording Cancel restores from - so `snapshot !== undefined` *is* "a draft is open", and there
is nothing to get out of step. The three actions name the period rather than the recording: `beginDraft` opens it,
`commitDraft` ends it by keeping it and `cancelDraft` ends it by discarding it. Ending is what reopens storage, and
`commitDraft`'s empty `set` is what flushes the whole draft in one write - the rows have been writing the store all
along, so there is nothing else for Save to apply.

*Alternative considered.* **Holding the draft in React state and writing the store only on Save.** Rejected: it is the
same guarantee at fifteen times the cost. Every row would read and write a draft object threaded down from the dialog
instead of the one field it owns, and the store would gain a second shape that means the same thing - which is the
duplication the rest of this design is spent avoiding. Gating the write is one adapter, in the file that already owns
what persistence means here.

### The language is read from the store at i18n initialization, through a leaf module

`i18n/languages.ts` is new and imports nothing: the thirteen tags, the endonyms, the guard and the detection. The
settings store imports it; `i18n/index.ts` imports the store to read the persisted language for `lng`.

That ordering is the reference's, and so is the constraint that produced it: *"Leaf module: it must import nothing
from the app, because `stores/settings.ts` imports `detectLanguage` from here and `i18n/index.ts` imports the store.
Anything added here that reaches back into the app closes that cycle."*

`i18n/index.ts` keeps `CATALOGUES` and keeps deriving `SUPPORTED_LANGUAGES` from it, because the existing parity test
asserts across exactly that map and its comment explains why reading the directory instead would check the wrong
thing. The new leaf's tag list is then a second spelling of the same thirteen - so the parity test gains an assertion
that the two agree, which is what makes the duplication safe rather than a place to drift. That is cheaper than the
alternative: making the leaf import the catalogues would make it not a leaf.

**Read at initialization rather than applied by an effect.** `zustand/persist` over `localStorage` rehydrates
synchronously, so the store's language is correct at the moment `i18n/index.ts` runs. An effect calling
`changeLanguage` on mount would work too, at the cost of one frame of English on every launch for every user who is
not English.

### The processor's four names are already named on the wire; the surface adds only their casing and order

The store holds `"auto" | "cpu" | "coreml" | "cuda" | "tensorrt"` - the wire's own spelling, from the
`SupportedProviders` type slice 6 wrote. The options are built by walking the report: automatic first, then the
providers it reports true, then the CPU, which is the reference's order (`Auto, TensorRT, CUDA, CoreML, CPU`).

The surface therefore carries exactly two things of its own: the display casing (`TensorRT`, `CUDA`, `CoreML`, `CPU` -
product names, not translatable) and that presentation order. Slice 6's goal that *"the frontend carries no list of...
provider names of its own"* is met in the sense that mattered - which providers exist is still the report's answer,
and an unknown fifth field is one that is not offered, exactly as `ipc/setup.ts` documents. What is added is how four
names the frontend already spells are shown and sorted, which is presentation and has to live in a front end.

`settings.performance.aiProcessor.description` takes the chosen provider as its i18next context, resolving
`description_tensorrt` and its siblings and falling back to the bare key. All six keys are already in all thirteen
catalogues.

*Alternative considered.* **Putting `ExecutionProvider` on the wire from `opai`.** Rejected for this slice: there is no
command that takes one, so it would be a type crossing a boundary nothing sends it across. It arrives with
`set_execution_provider`, in the slice that runs a model.

### The persisted key is `settings-storage`, with this application's value spellings

The key is the Wails application's, which is the convention `stores/enhancements.ts` already set (*"The Wails app's
key, unchanged"*). The **values** are not: the processor is stored as `tensorrt` rather than the Go enum's
`"TensorRT"`, because this codebase already spells it that way on the wire and two spellings of one provider in one
file is the kind of drift the rest of this design is spent avoiding.

A `settings-storage` written by the Wails application, if one is ever reachable, degrades rather than breaks: the
language, the analytics flag, the models and the qualities are already in compatible shapes, and a `"TensorRT"`
processor is not among those offered, so the guard that exists for a removed driver replaces it with the automatic
choice. That is the correct outcome, reached by a rule that was needed anyway.

### Export quality: four format keys the frontend names, and the reference's four starting values

`avif | heic | jpeg | webp`, with defaults 60 / 60 / 90 / 75 and bounds 1..100. Taken from the reference's
`utils/quality.ts`, whose comment is the reason the numbers are not one shared value: *"the scales are not comparable
across encoders, and 60 in libheif is a very different picture to 60 in libjpeg"*, and they are *"exactly what
`utils.EncodeImage` used to hardcode, so a user who never moves a slider gets byte-identical output to before the
setting existed."*

The screens show 50 / 80 / 90 / 80 in this dialog; those are the mock's illustrative numbers rather than a decision -
the screen's note claims only that *"each keeps its own default mark and remembered value"*, and the reference's are
the ones with an argument behind them.

**The four names are the frontend's own**, and unlike the model vocabulary that is correct here: `opai` re-exports
`rust_sak::image::ImageFormat`, but nothing puts it on the wire and no command in this application takes one yet, so
there is nothing to read them from. The label is `HEIC` while the library's variant is `Heif` - the container the user
picks, in the spelling the reference and the design both use.

**The slider draws its default as a mark.** shadcn's slider has no mark support; it is an absolutely positioned tick
at the default's position on the track, which is also what the screen's note asks for. The value is shown beside the
track in the monospace face, as the screen draws it.

### `sonner` for the toaster, mounted in `AppProviders`

One `<Toaster />`, mounted beside `LucideProvider` and `TooltipProvider` for the reason that file already gives: it is
*"a decision made once for the whole application rather than per call site"*, and a provider mounted inside a feature
is the failure mode it documents.

The alternative - a line of red text under the Logs row - was considered and is worse in the one case that matters:
the reveal is fired from a button, and the user's attention is on the file manager they expected to appear, not on the
row they pressed. A transient notice is also what every later screen needs (`toasts.unsupportedFiles`, the CPU
fallback notice, the export failures), so this is a dependency the application was going to take.

**Two hand-edits on the generated component.** shadcn's `sonner.tsx` imports `useTheme` from `next-themes`, which this
application does not have and must not gain: it forces dark app-wide in `style.css`, so the theme is a constant. The
import is removed and the theme pinned. This joins the list of edits every `shadcn add` in this crate needs - the
bogus `cn` package, the arrow functions, the named React imports, then `pnpm check` and `pnpm typecheck` - which is
recorded in `add-gui-1-foundation/design.md` and applies unchanged to `select`.

### The navigation list highlights what was clicked, and scrolls to it

Each row carries the id its entry in the section table declares, and clicking an entry scrolls that row into view;
the clicked entry is the highlighted one.

The structure is one data module read by both panes, as the reference's is, and for its stated reason: the id
*"doubles as the anchor id in SettingsList and the tree node id in SettingsMenu. Defining it once here is what stops
the two from drifting: a mismatch used to compile fine and just make the nav entry scroll nowhere."* Here that is
strengthened into the spec's second navigation scenario - a test walks the table and asserts every entry has a row and
every row an entry.

**Not a scroll spy.** The design's two screens show one entry highlighted and say nothing about scrolling driving it,
and the reference has no such behaviour. An observer that re-highlighted while the user scrolled would be inventing an
interaction, and it fights the click it is meant to follow.

The MUI tree becomes a plain list, as the screen's note instructs: every section was always expanded, so the disclosure
was decoration.

### Where the files go

`features/settings/` for the dialog, its nav, its section table and its four row shapes;
`components/ui/dialog-title-bar.tsx` for the 48px title bar, which is shared rather than the settings dialog's, because
the crop, export, faces and About dialogs all draw it and slice 6's design already named it as this slice's;
`stores/settings.ts` for the store; `i18n/languages.ts` for the leaf; `lib/enhancements.ts` for the seven families in
pipeline order.

That last one is the frontend's only list of domain names, and it is not model vocabulary: it is which enhancements
this application presents and in what order. `opai` says so itself - `Family::ALL` is documented as *"declaration
order... Not the order the catalogue publishes - that one is a presentation decision and is pinned where it is made"*,
and the catalogue's own order is neither the pipeline's nor the menu's. The reference keeps the same list for the same
reason: *"a single list because the two must agree: the add menu offering them in one order while the pipeline ran
them in another would be a silent inconsistency."* It is in `lib/` rather than in `features/settings/` because the
add-enhancement menu and the sidebar are its next two readers. It names `detection` nowhere, which is how the
catalogue's eighth family stays out of a dialog about enhancements without the catalogue being filtered on the wire.

## Risks / Trade-offs

**Upscale's default changes from the reference's.** → Declared in proposal.md, argued above, and cheap to reverse: it
is one function, and the day `opai` gains a published default it becomes a read of that field. The cost of being
wrong is that a user who never opens Settings gets Tokyo instead of Kyoto on their first upscale, which is a different
good model rather than a broken one.

**Fifteen tasks against a fifteen-task limit.** → No slack at all, and the reason the `opai` change was worth removing rather
than absorbing. The compression point if something overruns is the row shapes: select, switch, button and slider can
be split from the rows that use them without either half being half-wired. What must not be compressed is the store's
snapshot coverage, which is the one thing here whose failure is silent.

**A persisted store is a new class of state that can be wrong on a user's machine and right on ours.** → Every field
has a stated repair, and the repairs are the spec's own scenarios rather than defensive code: the language against the
shipped catalogues, the processor against the report, the qualities against the bounds, the models against the
catalogue at the point of drawing. The tests seed storage with each broken shape rather than asserting the happy path.

**`i18n/index.ts` importing the store makes initialization order load-bearing.** → It rests on `zustand/persist` over
`localStorage` rehydrating synchronously, which it does, and which is checked by a test that seeds storage and asserts
the application comes up in that language. If it ever stops being true the symptom is one frame of English, not a
crash - and the fallback is the effect this decision rejected.

**The thirteen language tags are spelled twice.** → In `i18n/languages.ts` and as the keys of `CATALOGUES`. Held
together by an assertion in the existing parity test rather than by care. The alternative closes an import cycle.

**Two more generated components, with the same five hand-edits and one new one.** → `next-themes` is the new one and
is the loud kind: it is a missing module, so it fails at install or typecheck rather than shipping quietly. The quiet
kind is the visual defaults, which is why the launch check at the end of the task list compares the dialog against
screens 16 and 16b rather than only asserting it opens.

**Fifteen rows is a lot of surface to verify by hand.** → The row shapes are four, not fifteen; each shape gets its
test and the rows get theirs against the table. The manual check is for what tests cannot see: the card's metrics, the
subheader bands, the slider marks and the fact that the language actually changes on Save.
