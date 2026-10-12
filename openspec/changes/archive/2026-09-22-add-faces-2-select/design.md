## Context

See proposal.md — Why for the motivation. What shapes this design is that slice 1 built every seam
this needs and deliberately left one field out of each of them, so almost every decision here is
about where the choice lives rather than about how to get the faces.

**What slice 1 already left in place.** `stores/faces.ts` holds `Map<string, { crop?: CropInfo;
faces: Face[] }>` keyed by identity, registered with `registerFileOwner`, and its own doc records
*"There is no deselection here. Choosing among the faces is the next slice."* `ipc/faces.ts` answers
`Face` values whose `bounding_box` is `snake_case` on a camelCase wire, with a written warning not to
"fix" it, *"which is what will make a face deselected by value compare correctly once a user can
deselect one"*. `hooks/useEnhancementRun.ts` holds `withFaces`, the one place the faces are resolved
on the way to `enhance`. `FaceRecoveryOptions.tsx` draws one control and says why the second is
missing.

**What the window already has for the rest.** `components/ui/dialog.tsx` and `DialogTitleBar`, both
carrying Crop/Rotate and Settings — `DialogTitleBar` was put in `components/ui/` in
`add-gui-7-settings-dialog` on the explicit argument that *"crop, export, faces and About all draw
exactly this bar"*, and this is the third of the four. `renditionFor(file, bound, crop)` serves a
bounded **framed** picture; the sidebar's miniature has asked for one since `add-crop-3-dialog`.
`framedDimensions(file, crop)` in `ipc/crop.ts` answers the framed photograph's own dimensions, which
is what `NavbarDimensions` reports.

**What the catalogues already hold.** `faces.selectFaces`, `faces.toggleFace`,
`enhancements.faceSelector.title/help/button` and `enhancements.faces_partial_one/other`, present and
translated in all thirteen locales. The reference's `facesLabel` composes the last of those with
i18next's `context: 'partial'`, which is the mechanism this follows.

**What the reference does, read from the source.**
`cmd/gui/frontend/src/features/faces/FaceToggle/index.tsx` fits a MUI dialog to the photograph's
aspect by measuring `window.innerWidth/innerHeight` behind a resize listener coalesced to one update
per animation frame, draws `FaceBoxes` — absolutely positioned buttons scaled from original-image
pixels — and commits the working copy from `handleClose`, so every way out is a commit.
`hooks/useFaceSelection.ts` keeps `Set<number>` of indices and writes only when the set actually
changed. `utils/face.ts`'s `getEnabledFaces` filters `faces.filter((_, i) => !d.has(i))`.

## Goals / Non-Goals

**Goals:**

- The choice lives with the photograph's faces, is expressed in the faces themselves, and needs no
  reset rule when the faces are found again.
- The dialog costs one run per Apply and none per box.
- The boxes need no measurement, so the whole surface is testable under jsdom.

**Non-Goals:**

- **No boxes on the canvas.** Settled during slice 1's exploration and recorded there; screen 13's
  note says otherwise and no screen in the design file draws them, and the reference draws them only
  in its dialog. `gui-faces` now states it as a requirement so it stops being a note in an archive.
- **No fidelity control.** Fixed at maximum, not on the wire, argued in slice 1's D7.
- **No selection carried across photographs**, and none persisted. Faces belong to a photograph that
  is open.
- **Nothing in Rust.** See D1.

## Decisions

### D1 — No Rust, and nothing missing for it

The wire already carries what this needs. `Requested::FaceRecovery` takes `faces: Vec<Face>` and
`enhance` runs over exactly those; `face-recovery` already requires that a result stored for one
selection is not served for another, because `Faces::write_cache_signature` folds an order-sensitive
signature of the bounding boxes into the run cache tag. So sending three faces where four were found
is already a different run with a different result, with no library change and no new requirement
below the window.

*Why this is not logic belonging in `opai`.* Which of an open photograph's faces a user is currently
interested in is a property of a window with that photograph open and a chooser on screen. The CLI
names its own operations and passes its own faces; it has no chooser, no open photograph and nothing
to remember a choice for. This is the same test `gui/src/faces.rs` already passed in slice 1 and the
same answer.

*`rust-sak`:* nothing used and nothing missing — no Rust is written here at all.

### D2 — A skipped face is recorded by value, in a set beside the faces

```ts
faces:   Map<string, { crop?: CropInfo; faces: Face[] }>   // slice 1, unchanged
skipped: Map<string, ReadonlySet<string>>                  // identity -> face keys
```

The key is derived from the face's **bounding box** — its four coordinates, in the framed
photograph's pixels — which is the same thing `Faces::write_cache_signature` folds and is a face's
identity as far as anything in this application is concerned. A `Face` is quantized to a hundredth of
a pixel on acceptance, so the key is stable across the re-detections that matter.

**By value rather than by index**, which is the decision settled with the repository owner during
slice 1's exploration and recorded in its archived design. The whole benefit is that *there is no
reset rule*: a face that comes back keeps its state, one the framing removed matches nothing, and one
newly found is on, all as a consequence of the lookup rather than of an effect someone has to write.
A framing nobody has been at before therefore leaves every face in it undecided — the coordinates all
moved, so nothing matches — and the default that applies to an undecided face is D12's size rule. A
framing *returned to* restores the choice, because slice 1's run store answers it with the faces
found there the first time, byte for byte, so the keys match again. D12 has to hold the record of
what has been decided the same way for that to be true of a face the user turned *on* as well as of
one they turned off — see the `decided` set there.

**Keyed by identity and held in `stores/faces.ts`**, not `stores/enhancements.ts`. The choice is
about the pixels, not about the operation: `gui-faces` requires that removing a face recovery and
adding another leaves the choice alone, and a map keyed by the file's *path* beside the stack would
make that an effect rather than a fact. It rides `registerFileOwner` with the faces, so a close
releases both.

**Not keyed by framing**, which follows from the by-value rule: a skipped key from a framing nobody
is at matches nothing and costs one string. The set grows only by faces a user actually skipped and
is released on close, so it is bounded by what a person can click.

*Alternative considered:* the reference's `Set<number>` of indices. It is smaller and it survives a
framing change — but it survives it by *position*, so cutting the first face out of a photograph
promotes the second into the skipped slot and the run silently stops restoring a face the user never
touched. Its own `utils/face.ts` comment shows what it is defending: indices are valid across a tier
switch *only* because the two detector builds find the same faces in the same order. That is a
narrower guarantee than the one this needs.

### D3 — `lib/faces.ts`: the key, and the enabled set

Four functions. Two because three call sites need them and none owns the other two:
`hooks/useEnhancementRun.ts` (what the run carries), `EnhancementRow.tsx` (what the line reports) and
the dialog (what is yellow). The other two are D12's rule and the record it reads, which have one
caller between them — the store — and are here rather than inside it because they are arithmetic over
the wire's own shape, testable without a store.

```ts
export const faceKey = (face: Face): string      // the four bounding-box coordinates
export const enabledFaces = (faces: Face[], skipped?: ReadonlySet<string>): Face[]
export const decidedSkips = (                    // D12: what a detection is recorded with
    found: Face[],
    decided: ReadonlySet<string> | undefined,
    skipped: ReadonlySet<string> | undefined,
): ReadonlySet<string> | undefined
export const withDecided = (                     // D12: and what has been asked about, for next time
    found: Face[],
    decided: ReadonlySet<string> | undefined,
): ReadonlySet<string> | undefined
```

`enabledFaces` returns `faces` **itself** when nothing is skipped, rather than a copy. That is not a
micro-optimisation: `useEnhancementRun`'s effect lists the faces among its dependencies, and a fresh
array on every render would re-run the chain on every render. The filtered array is built inside the
effect for the same reason — see D7.

In `lib/` rather than in the store, beside `lib/enhancements.ts`, because it is a derivation of the
wire's own shapes that three regions read and no region owns — the same argument that file makes for
`upscaleFactor`.

### D4 — The choice is edited in a working copy and committed by Apply

`features/faces/useFaceSelection.ts` holds a `Set<string>` seeded from the store each time the dialog
opens, a `toggle`, and an `apply` that writes the store only when the set actually differs from what
is committed. Dismissal writes nothing at all, which is the whole of the discard — there is nothing
to unwind, which is the property `useCropController` has and `useSettingsStore` spent a redesign to
get.

**This is a deliberate divergence from the reference**, which commits from `handleClose` and so has
no discard at all. Screen 13b draws a footer with an Apply carrying the count, and an Apply button
means nothing unless dismissal is the other answer. It is also the shape Crop/Rotate already ships in
this application — Apply commits, Escape and the close box do not — so the two dialogs a user can
open over a photograph answer dismissal the same way.

**Why the working copy exists at all**, in both implementations: writing the store per box clicked
would start an inference run per box. The reference's own comment says so — *"Commit the working
selection to the store only on close, so toggling many faces triggers a single inference."* The
"only when changed" guard is the reference's too, and it is what makes opening the dialog to read the
selection and closing it leave the run in flight alone.

### D5 — The boxes are percentages of the framed photograph, so nothing is measured

A box is positioned with `left/top/width/height` in **percent**, computed from the face's coordinates
over `framedDimensions(file, crop)`. The picture element is drawn at exactly the framed photograph's
aspect, so a percentage lands in the same place whatever size the element happens to be.

This removes the reference's entire measurement apparatus: no `window.innerWidth/innerHeight` state,
no resize listener, no per-animation-frame coalescing, no `displayWidth`/`displayHeight` props, and
no `useResizeObserver`. It also makes the surface testable: jsdom has no layout, so a box positioned
from a measured element is a box at `0,0,0,0` in every test, while a percentage is in the style
attribute and can be asserted.

**Where the framed dimensions come from.** `framedDimensions` answers the crop rectangle's own
width and height for a framed photograph — always present, since Rust refuses a rectangle with no
area — and the file's own dimensions otherwise. The one case it cannot answer is a photograph whose
header this application could not parse and which carries no framing; there the loaded rendition's
`naturalWidth`/`naturalHeight` are used instead, which is exact whenever the photograph is under the
dialog's bound and is the best available otherwise. It is a corner of a corner — the file decoded but
did not measure — and the failure is boxes drawn at the wrong scale in a dialog, not a wrong run.

### D6 — The dialog fits the photograph in CSS, and asks for a bounded framed rendition

The mockup's dialog is `top:32; bottom:32`, its width following the picture — 634px for a square
picture, which is 628px plus 2px of padding on each side. No JavaScript measures the viewport for
any of it, which is the other half of D5's saving.

**A shrink-wrap does not survive the round trip, so both edges are written against the viewport.**
The obvious arrangement — the picture box carrying `aspect-ratio` from the framed dimensions with
`max-h-full max-w-full`, and the dialog shrink-wrapping around it between the 48px `DialogTitleBar`
and the 52px footer — is circular, and a browser resolves that circle by dropping the aspect: the
picture is asked to fill a height that is not yet definite because the dialog is still waiting on the
picture to say how wide it is. What is drawn instead is a letterboxed picture with the overlay
standing over the bars, and the boxes over it are at percentages of an element that is no longer the
photograph's shape — so it is a wrong answer rather than an ugly one.

`100vh` and `100vw` are definite before any of this lays out, so there is no circle to resolve. One
exported function answers both edges from them:

```ts
export const pictureFit = (width: number, height: number) => ({
    picture: `min(100vh - ${VERTICAL_CHROME}px, (100vw - ${HORIZONTAL_CHROME}px) * ${height} / ${width})`,
    dialog: `calc(min(100vw - ${HORIZONTAL_CHROME}px, (100vh - ${VERTICAL_CHROME}px) * ${width} / ${height}) + ${PICTURE_FRAME}px)`,
});
```

Both terms are the same `min` of the two edges — the window's height less the chrome, and the
window's width less the chrome converted through the photograph's own proportions — so whichever
edge binds binds in both, and the picture is the photograph's shape at whatever size that leaves. The
chrome budgets are the numbers written in the markup and in `DialogTitleBar`, added up in one place
rather than spread through the classes. A `min-w-min` floor sits under the width for the one case the
arithmetic does not cover, a tall narrow photograph in a short window, where the dialog would
otherwise come out narrower than its own footer; being `min-content` it is whatever the footer needs
in the language being spoken rather than a number measured once in English.

It is exported for its own test: a browser and jsdom serialize a `min()` expression differently, so
the arithmetic is pinned where it is a returned string rather than through a style attribute where it
is whichever parser read it.

**The picture is the framed rendition**, `renditionFor(file, FACES_DIALOG_BOUND, crop)` — the faces
are in the framed photograph's pixels, so a chooser drawing the unframed file would put every box in
the wrong place, and a face the crop removed over nothing at all.

**`FACES_DIALOG_BOUND = 2048`**, derived rather than picked, the way `THUMBNAIL_BOUND` and
`CROP_DIALOG_BOUND` are. The picture box is the window's height less the 64px inset, the 48px title
bar and the 52px footer — about 916 CSS pixels on a 1080-tall window — and the window's width less
64. 2048 covers a ~1024 CSS-pixel picture at 2x density, which covers that window; a much taller
display draws it slightly soft. That trade is taken deliberately: this is a surface for telling faces
apart and clicking them, not for judging restoration detail, and the alternative is what the
reference does — a full-resolution decode, JPEG encode, transfer and second decode of a 9504x6336
scan to fill a box a fifth of its size.

One fixed constant rather than one derived from `devicePixelRatio`, for the reason `THUMBNAIL_BOUND`
gives: the bound is in the URL, so a bound that moved with the display would re-request the
photograph when the window was dragged to another monitor.

### D7 — `useEnhancementRun` filters inside the effect

```ts
const skipped = useSkippedFaces(source);   // the store's own selector, beside useImageFaces
...
const chosen = enabledFaces(faces ?? [], skipped);
const { run, done } = enhance(source, withFaces(operations, chosen), processor, crop);
```

`skipped` is a dependency; `chosen` is built **inside** the effect. The store replaces the whole set
on a write and hands back the same reference until one happens, so the dependency changes exactly
when a choice is applied and at no other time — which is what makes "a committed change re-runs the
preview, once" fall out of the existing effect rather than needing a second one. Computing `chosen`
in the render body and listing *it* would list a fresh array on every render and re-run the chain on
every render.

`withFaces` is unchanged: it still puts whatever array it is handed into every face recovery in the
stack. What changed is which array it is handed, which keeps the one place the faces are resolved as
the one place — slice 1's D5.

The detect-then-enhance ordering is untouched. A choice cannot exist before the faces do, so the
first pass still detects, the store write still brings the effect back, and the second pass enhances
over the chosen subset.

### D8 — The dialog is mounted beside the popover, not inside it

`EnhancementRow` holds the open state and renders `<SelectFacesDialog>` as a **sibling** of
`<Popover>`; `FaceRecoveryOptions` takes an `onSelectFaces` callback and the row closes the popover
when the dialog opens.

The reference mounts its `FaceToggle` inside `OptionsPopover` and gets away with it because MUI's
popover is not dismissed by a modal opening over it. Radix's is: a dialog takes focus and pointer
capture outside the popover's content, the popover closes, `FaceRecoveryOptions` unmounts — and the
dialog unmounts with it, mid-open. Lifting the state one level makes the dialog's lifetime the row's
rather than the panel's, and closing the popover deliberately means there is one thing on screen
rather than a panel peeking out from behind a modal.

*Alternative considered:* keeping the state in `FaceRecoveryOptions` and forcing the popover to stay
open by preventing its dismissal handlers. That trades a clear ownership line for two dismissal rules
that have to stay in agreement, and leaves a panel drawn behind the dialog that the design does not
draw.

The cost is one family-specific line in a component that is otherwise family-agnostic. It is the same
carve-out the row already makes for the info line and the options panel, both of which switch on the
operation's family.

### D9 — The info line reports the ratio through the catalogue's own `context`

```ts
t("enhancements.faces", { count: faces.length, enabled, context: enabled === faces.length ? undefined : "partial" })
```

The reference's `facesLabel` exactly, and the reason is in its own comment: i18next composes the
suffixes as `key_context_plural`, so `faces_partial_one` and `faces_partial_other` are selected
without a second call site knowing they exist. **Pluralising on the total rather than on the enabled
count** is the reference's choice and is kept — it is what makes `0/2 Faces` read correctly in
English, and a language with more plural forms adds keys to its own catalogue with no code change.

Slice 1's rule stands above this: a photograph whose faces are **not known yet** reports the model
and the quality alone through `infoModel`, because `0 Faces` would look like an answer. Only a known
set is counted, and only a known set can be chosen from.

### D10 — Two new catalogue keys, and only two

`faces.hint` — *"Click a box to include or skip a face."* — and `faces.apply_one/other` — *"Apply to
{{enabled}} of {{count}} faces"*, pluralised on `count` as D9's is.

These are the mockup's footer, which the reference has no dialog chrome for at all, so the catalogue
carried no string for them. Everything else these screens need is already in all thirteen locales
and is used unchanged: `faces.selectFaces` titles the dialog, `faces.toggleFace` names each box,
`enhancements.faceSelector.title/help/button` draw screen 13's block — `help` through `<Trans>`,
because it carries a `<br/>` that a translator is meant to be able to move, which is i18next's own
default kept node.

### D11 — One movement of the bar across the detection and the chain it was run for

Not a decision this slice went looking for. Slice 1 drew each run's own `chainFraction` as it
arrived, and a face recovery is two runs: the detection fills the bar, the chain empties it and fills
it again. Two sweeps for one piece of work reads as the enhancement having been applied twice, and
this slice is what makes it constant rather than occasional — a chooser is a reason to re-run a
recovery over and over, so the double sweep stops being something a user sees once.

**Composed in `useEnhancementRun`, which is the only place both runs are known.** The detection owns
the head of the range and the chain the rest; a chain that needed no detection owns the whole of it,
including one over faces already known, because only the chain a detection was *just* made for has a
head to begin after. The head is consumed as it is read and matched on the photograph and the
framing, so a detection the window has moved off does not hand its tail to whatever runs next.

**`ProgressBar` is given the figure rather than reading it.** `report.chainFraction` describes
whichever run the report came from, so a bar drawing it is a bar drawing one of the two. Handing it
down makes the composition the hook's and leaves the bar drawing what it is told — and the prop is
required rather than optional, so no caller can fall back to the half-answer by omission.

**And a detection that lands for a stack that no longer restores faces is kept, not acted on.** It is
the same effect and the same read: the answer is recorded for the framing it was found at, and the
run in flight over the remaining enhancements is neither stopped nor started again. Nobody asked that
question of the list as it now stands, and re-asking it would throw away work already done and take
this very bar back to the start.

*Why it is in this slice rather than its own.* It is two lines of composition in an effect this slice
already rewrites for D7, and a prop on a component with one caller. Cutting it out would mean a slice
whose whole content is a fraction, ordered after the one that makes the defect constant.

### D12 — A face starts chosen or skipped according to its area, and the store decides it

The default for a face is not "chosen". It is **the area its bounding box covers**: above 512x512
pixels the face starts skipped, at or below it the face starts chosen.

512x512 is the tile the recovery models actually run at —
`crates/opai/src/models/face_recovery/restore.rs` exports its weights at a static `[1, 3, 512, 512]`
and both shipping variants align a face to a 512-pixel template. A face whose box already covers more
than that is reduced into the tile, restored, and enlarged back, so what the model can give back is
bounded by what the reduction threw away; on a face that was already sharp it is a softening. Below
the tile the model works at or above the detail the photograph holds, which is where a restoration is
worth having. The number is therefore read off the models rather than chosen, and it moves if they do.

Compared as an **area** rather than edge against edge, which is what the repository owner asked for: a
1024x256 box and a 512x512 one are the same amount of face to restore.

**A face exactly at the tile is chosen.** It is neither reduced nor enlarged, so the argument above
begins strictly above the number.

**In the store's `setFaces`, not in the run path.** The rule needs two things the store already holds
in one place — which faces have been decided, and the choice standing for the photograph — and it
must write the faces and the choice in **one** `set`. `useEnhancementRun` lists both among its
effect's dependencies (D7), so two writes would give it one render holding the new faces beside the
old choice: a chain started over every face found, cancelled by the next render and started again.
Seeding from `record()` in that hook is exactly the two-write shape.

**It decides only the undecided**, which is what `decided` is for: a third map beside the faces and
the choice, holding the key of every face the photograph has **ever** been asked about. A face whose
key is in it has already been decided, by this rule or by the user since, so a re-detection at the
framing in force changes nothing at all. A face at coordinates nothing has been asked about is decided
here, which covers the first detection, a framing nobody has been at, and a photograph closed and
opened.

**Ever, rather than the faces this write replaces**, and this is the part that is easy to get wrong:
the faces map holds *one* detection, the one at the framing in force. Read against that alone, a face
stops being decided the moment another framing has been visited — so a user who turns a large face on,
crops, and undoes the crop comes home to it skipped again, the size rule having silently undone a
choice they made by hand. `gui-faces` forbids exactly that (*"a face the user has decided SHALL keep
what the user made of it"*), and D2's whole framing-returned-to property depends on it holding for a
face turned on as much as for one turned off. `withDecided` accumulates the record; `setFaces` reads
it **before** folding this detection into it, since a set that already held the new faces would decide
none of them.

*Cost of accumulating:* one short string per face per framing visited, never pruned while the
photograph is open — because a key dropped on the way out of a framing is a key missing on the way
back into it — and released with the faces and the choice when it is closed. It is the same bound the
choice beside it has and the same release.

**It only ever adds.** A small face the user skipped by hand is not turned back on by being found
again, or a crop elsewhere in the photograph would restore a face the user asked to be left alone —
and D2's framing-returned-to property would stop holding.

**It writes nothing when it adds nothing**, and reports that by handing back the set it was given. A
set replaced with an equal one is a new reference, and a new reference is precisely what D7's effect
reads as a choice the user just applied.

*Alternative considered:* leaving the rule to the first detection only, so a framing change goes back
to "everything chosen". Settled with the repository owner the other way: the default that applies when
a framing change drops the old choice is this rule, because a default that disagrees with itself
depending on how the photograph is framed is two defaults.

*Alternative considered:* deriving it at read time — no seeding, and `enabledFaces` applying the rule
wherever the skipped set is absent. It cannot express a user turning a large face **on**, because "on"
is spelled as absence from the set. Seeding is what makes the user's choice and the default the same
kind of thing.

## Slices

Face recovery in the window, in order. This is the last slice; the series closes with it.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-faces-1-seam` | The `face_recovery` wire arm; the `detect_faces` command; `ipc/faces.ts` and `stores/faces.ts`; Face Recovery in the add menu; the detect-then-enhance run path; the `2 Faces` info line; detection reported on the existing indicator. Restores **every** face found | - | **done** (archived 2026-09-22) |
| 2 | `add-faces-2-select` | Screen 13's Faces block in the options panel; screen 13b's Select faces dialog with its boxes, footer hint and Apply; the deselection; the `1/2 Faces` info line; the re-run on a changed selection; one movement of the progress bar across a detection and the chain it was run for | 1 | **current** |

### Decisions inherited from slice 1's archived design, and what became of them

Settled with the repository owner during slice 1's exploration and recorded there so this slice would
inherit them from a document rather than from a conversation.

- **Face boxes are drawn in the Select faces dialog only**, not over the Original pane. *Acted on:
  Non-Goals, and now a requirement in `gui-faces`.*
- **A deselection is stored as the `Face` value, not as an index.** *Acted on: D2.*
- **The Select faces dialog is fitted to the photograph's aspect, and Apply commits while the close
  box and Escape discard.** *Acted on: D4 and D6.*
- The detector running at FP32 always, detection reported under the Face Recovery name, and fidelity
  fixed at 1.0 and off the wire were all acted on in slice 1 and are untouched here.

## Risks / Trade-offs

**A framing change discards the choice** → By design, and stated in `gui-faces` rather than left to be
discovered: the faces in a re-framed photograph are different faces at different coordinates, so
every one of them is decided afresh by D12's size rule. The mitigation is the one that makes it
acceptable — a framing *returned to* restores the choice exactly, because slice 1's run store answers
a repeated framing with the same faces. A user who crops and undoes the crop does not lose their selection; one who genuinely re-frames
is looking at a different set of faces and is asked about them afresh.

**A skipped-key set accumulates keys for framings nobody is at** → Bounded by the faces a person has
actually clicked, and released on close with everything else the photograph owns. It is a set of
short strings, not a cache of pictures.

**The decided-key set accumulates faster than that** → It grows by every face found at every framing
visited, not only the ones clicked, and D12 argues why it may not be pruned: a key dropped on the way
out of a framing is a key missing on the way back into it, and the size rule would then undo a choice
the user made by hand. Still short strings, still one photograph's worth, still released on close. A
person would have to re-frame a crowd scene hundreds of times to make it worth measuring.

**The bounded rendition makes the dialog's picture softer than the reference's** → D6 takes the trade
deliberately and gives the arithmetic. The correctness of the boxes does not depend on it: they are
percentages of the framed photograph, so a softer picture is a softer picture and nothing else.

**jsdom cannot check the fit** → The aspect fit is CSS, and jsdom has no layout, so the tests can
assert the `aspect-ratio` and the `min()` expressions that were asked for and nothing about what they
produced. The by-hand check in
tasks.md is the only coverage the fit gets, and it is written to cover a portrait, a landscape and a
square photograph, since those are the three ways a shrink-wrapped aspect box goes wrong.

**A face too small to click** → The boxes are the detector's own, and a face a few pixels across in a
large photograph is a target a few pixels across in the dialog. The reference has the same property
and no mitigation; this slice does not add one either, and it is recorded here so a later change can
pick it up rather than rediscover it.

**Platforms:** the frontend is one build on all three. The by-hand verification is on macOS, as every
GUI slice in this project has been; Windows and Linux are untested for this slice, and nothing here
is platform-conditional — no execution provider, model session or memory budget is touched.
