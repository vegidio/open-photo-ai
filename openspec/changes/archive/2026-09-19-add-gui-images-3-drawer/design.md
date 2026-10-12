## Context

See proposal.md - Why. What shapes this slice is that everything it needs already exists on both sides
of the boundary: the strip is a list the store already holds, drawn from URLs `renditionUrl` already
builds, at a bound the protocol has accepted since slice 1.

`gui` has, and this slice adds nothing to: `open_images`, `describe_images`, `input_extensions`, the
`opai` URI scheme serving an admitted image's pixels bounded on its longest edge, and
`renditionUrl(identity, bound)`. No Rust is touched.

The frontend has `stores/files.ts` - the open records in order, a `currentIndex`, one `addFiles`
writer and a `useCurrentFile` accessor - whose own comments record the two things left for this slice:
selection "is written by the slice that draws the checkboxes, against the strip that reads it", and
nothing sets `currentIndex` because nothing yet asks. `Drawer.tsx` holds `const open = false` with a
comment saying the fold "is disabled because there is nothing to unfold onto", `DrawerHeader` carries
the fold toggle and Select all both hard-disabled, and the drawer's body is an empty
`bg-secondary` rectangle 128px tall. `components/ui/sonner.tsx` offsets notices by the drawer's folded
height, which is the right number only while the drawer cannot unfold.

The reference implementation draws this strip from `DrawerBody`, `DrawerItem`, `DrawerSelectAll` and
`DrawerToggle` over `useFileStore` and `useDrawerStore`, windowed with `@tanstack/react-virtual` and
deferred with a `useThumbnail` hook built on an `IntersectionObserver`. Its comments are the record of
what it measured on a 500-file drop, and D3 and D5 are where this design keeps its conclusions and
drops its mechanisms.

## Slices

The four slices that take the application from an empty canvas to an open, zoomable image. This is a
new series; the `add-gui-1-foundation` .. `add-gui-7-settings-dialog` series closed out on the Settings
pair and is not continued here.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-images-1-ipc` | `opai::image::resize`; the picker command and the describe-paths command; the image record; the identity-addressed rendition protocol and the opened-file registry behind it. No UI | - | **DONE** |
| 2 | `add-gui-images-2-canvas` | Browse images and drag-and-drop wired to slice 1; the file store; the canvas drawing the current image in all three preview modes; the navbar's filename and Dimensions; the sidebar miniature | 1 | **DONE** |
| 3 | `add-gui-images-3-drawer` | Screens 08 and 08b - the thumbnail strip, its windowing and deferred decode, the per-item checkbox and name bar, Select all, the current-file outline, switching images, and the fold toggle made live | 2 | **CURRENT** |
| 4 | `add-gui-images-4-zoom` | The per-image transform shared across panes, the drawer's zoom slider and the wheel/pinch zoom, split mode's draggable handle, and the viewport rectangle drawn over the sidebar miniature | 3 | pending |

**Slice 3 is this change.** It answers the question slice 1 opened and slice 2 declined - what bound
the strip's thumbnails ask for - in D4.

**Crates.** `gui` only, and within it the React frontend alone. No Rust file is edited: see
proposal.md - Impact for why none of this is shared logic the terminal front end could want.

**rust-sak: nothing used directly, nothing missing.** This slice decodes, encodes and hashes nothing.
Every thumbnail is a URL served by the protocol handler slice 1 built, which reaches `rust-sak` through
`opai` exactly as it already did.

**New crates: none. New npm packages: one**, `@tanstack/react-virtual` - D3.

**Platforms.** Written for all three, validated on macOS. Two things are platform-shaped and neither
is a correctness question: whether a horizontal scrollbar takes height out of the strip, which D9
measures rather than predicts, and whether the webview honours `loading="lazy"`, which R2 bounds.

**Execution providers: none are exercised.** Nothing here builds a session or runs a model.

## Goals / Non-Goals

**Goals:**

- A strip that costs what is on screen, not what is open, so that the 500-file drop the reference
  tuned for is a supported thing to do here from the first version rather than after a later fix.
- A selection the export change can read without revising it - one set, one meaning, keyed the way the
  file list is already keyed.
- Every region that reports "which photograph" keeps reading `useCurrentFile`, so switching images is
  one store write and nothing else.

**Non-Goals:**

- **The file options menu** (screens 09 and 09b) - proposal.md, Impact. Its three-dot control is drawn
  in the name bar as a glyph, not as a disabled button, for the reason in D6.
- **Anything that removes a file.** Closing an image is what would empty the store, and it arrives with
  the menu above. The selection therefore never has to survive a file disappearing from under it.
- **The zoom slider and the per-image transform** (slice 4). The slider stays disabled beside a fold
  toggle that now works, which is the same honest intermediate state slice 2 shipped.
- **Anything acting on the selection.** Checkboxes record a choice that nothing yet reads; the export
  queue is a later change and is the reason the choice is recorded at all.
- **Keyboard navigation between images** - arrow keys moving the current image along the strip. The
  reference has none, the design draws none, and each thumbnail is a real button, so the strip is
  already reachable and operable by tab and space.
- **Naming a per-file failure.** A file whose pixels cannot be served draws as the empty thumbnail
  surface the design already puts behind every image, with its name bar intact (D10). Telling "could
  not be decoded" apart from "has not loaded yet" needs a state the `<img>` element does not report.

## Decisions

### D1. The drawer's open state is its own store, and the strip's fold is the only thing in it

`stores/drawer.ts`, holding `open` and the two writers over it. Not persisted - whether a drawer is
folded is a property of the session, and the reference does not persist it either.

A store rather than `useState` in `Drawer`, because three things outside the drawer's own subtree read
or write it: the toaster clears the strip (D8), the effect that unfolds it watches the open image count
(D7), and the fold toggle lives in `DrawerHeader` rather than in `Drawer`. Lifting it into `App` would
put a piece of drawer state in the shell and thread it through two components to reach the toaster,
which is mounted in `providers.tsx` and is not under `App` at all.

**The zoom is not in it.** The reference's `useDrawerStore` holds `open` and `zoom` together, which is
an artefact of both controls sitting in the same header: the zoom is a property of how an image is
being *looked at* and belongs with slice 4's per-image transform, not with whether a strip is showing.
Slice 4 decides where it lands; this store does not pre-empt it.

### D2. Selection is a set of paths in the file store, and only a set

`stores/files.ts` gains `selectedPaths: Set<string>` beside `files`, with `toggleSelected(path)`,
`selectAll()` and `unselectAll()`.

**One structure, not the reference's two.** It keeps `selectedFiles: File[]` *and* `selectedPaths:
Set`, and its own comment says the Set exists "purely so a membership test is O(1)" - every drawer item
subscribes with its own "am I selected?" check, and scanning the array made one Select all cost a
quarter of a million comparisons at 500 files. The array is the part that has no second reason to
exist: the only consumer that wants selected *records* is the export queue, and it wants them once,
when it opens, where `files.filter((file) => selectedPaths.has(file.path))` is one pass over an array
that is already in hand. Keeping both means every writer updating both, which is a class of bug for a
pass the application performs once per export.

**Keyed by path, not identity.** Identity is optional on a record - a file whose bytes could not be
read has none - and `addFiles` already de-duplicates by path, so paths are exactly the unique keys of
the list. Two copies of one photograph in different folders are two rows in the strip and two
independently pickable files, which identity would have collapsed into one.

**Replaced rather than mutated.** No `immer` here, where the reference has it: a `Set` mutated in place
is the same object, so Zustand's equality check sees no write and no subscriber re-renders. Each writer
builds a new `Set` from the old one, which is the plain-Zustand idiom and is what the existing stores
in this codebase already do with arrays.

*Alternative: a `selected: boolean` on each record.* It makes "is this picked" a property of the row
the strip is already rendering and needs no second structure at all. It was refused because the records
are what Rust described: `ImageRecord` is the shape of an IPC answer, and growing a UI flag on it makes
the boundary type and the store's row the same type by accident. Select all would also have to rewrite
every element of `files`, re-rendering every mounted thumbnail, where replacing one `Set` re-runs a
boolean selector per item and re-renders only the ones whose answer changed.

### D3. The strip is windowed with `@tanstack/react-virtual`, laid out by two spacers

The mounted set is the visible range plus an overscan of 4; two flex children stand in for the items on
either side that are not mounted, sized from the virtualizer's own numbers.

This is the design's own statement of screen 08b - *"windowed (@tanstack/react-virtual) with two
spacers standing in for the unmounted items"* - and the reference's implementation, and the reason to
keep both is in the reference's comment: the spacers leave the strip's flex, padding and gap exactly as
they were, so item size, spacing and padding are not this component's arithmetic to get right.
Positioning items absolutely meant predicting the height a scrollbar leaves behind, and every version
of that prediction was off by a few pixels.

*Alternative: mount every thumbnail.* Ours are lighter than the reference's - a button, an `<img>`, a
Radix checkbox and a span, against MUI's Checkbox, IconButton, Typography and Menu - so 500 files is
perhaps 2,000 nodes rather than 2,500. That is the same order of magnitude as the number the reference
measured and rejected, and it is not the expensive part anyway: every item subscribes to the file store
with its own selector, and Zustand re-runs every subscriber's selector on every write, so one Select
all click runs 500 selectors on a synchronous path. Windowing cuts both to what is on screen.

*Alternative: `content-visibility: auto`.* It skips layout and paint for off-screen items with one CSS
property and no dependency, and it does nothing about mounting, subscribing or the store writes above,
which is the cost that matters.

*Alternative: `react-window`.* The same idea with an absolute-positioning API, which is precisely the
layout the reference's comment records fighting.

### D4. A thumbnail asks for the photograph bounded at 384

One constant for every thumbnail on every machine.

The arithmetic: the strip's body is 128px with 12px of vertical padding, so an item is a 104px square,
and the design fills it *cropped* - `background-size: cover`. A cover crop is driven by the **short**
edge, so on a 2x display the short edge must reach 208 device pixels; bounding the **long** edge to
reach that takes 312 for a 3:2 photograph and 370 for 16:9. 384 clears both, is a round number, and is
the same number on every machine - which matters because the bound is in the URL, so a bound that moved
would be a second cache entry for a photograph already decoded.

*Alternative: the reference's 100.* It is what `useThumbnail(file, 100)` asks for, and it is the
strongest argument available for any other number - except that it was chosen when a thumbnail was
base64 over a Wails IPC channel, where every byte was paid twice, and it is below the item's own CSS
size before density is considered at all. Following it would ship a visibly soft strip on every Retina
Mac, which is the machine this is developed on. This is the divergence proposal.md, Impact records.

*Alternative: a bound derived from `devicePixelRatio`.* It is the exactly-right number and it makes the
URL machine-specific: a window dragged from a Retina display to an external monitor re-requests every
thumbnail at a new bound, and the webview's cache - which slice 1 chose deliberately over an LRU of our
own - holds two renditions of every photograph.

*Alternative: no bound, reusing the canvas's URL.* Free for the one image the canvas is drawing and
ruinous for the other nineteen: slice 2's D7 already drew the line here, and this strip is the case it
drew it for.

### D5. Deferring a thumbnail's pixels is `loading="lazy"`, not an observer

The `<img>` carries `loading="lazy"` and nothing else defers it.

The reference needs `useThumbnail` because a thumbnail there is an IPC round trip returning bytes that
become an object URL which must later be revoked; nothing but an `IntersectionObserver` knows when to
start that, and the hook exists to hold the observer, the cancellation and the unmount guard. Slice 1
replaced that whole channel with a URL, and lazy loading is the platform's own version of the same
deferral, on the same trigger, with no hook, no state, no cancellation and no revoke to get wrong.

It composes with D3 rather than duplicating it: the virtualizer bounds how many thumbnails are
*mounted*, lazy loading bounds which of those are *fetched* - the overscan items are mounted
deliberately, and there is no reason to decode one until it is nearly visible.

*Alternative: port `useThumbnail`.* It would be an `IntersectionObserver` per item to decide when to
set a `src` the browser would then decide about a second time.

### D6. A thumbnail is one button, with the checkbox beside it rather than inside it

The item is a positioned wrapper holding two siblings: a `<button>` covering the square - the image and
the name bar - and the checkbox overlaid on its top right corner.

The reference makes the item a `<div>` with an `onClick`, its own comment saying it is "to avoid nested
buttons", and suppresses two accessibility lint rules to do it. Siblings avoid the nesting without
avoiding the semantics: the item is a real button, so it is tabbable, operable by space and enter, and
announced as a control, and the checkbox is a real checkbox rather than a control inside a control.

The button's accessible name is the file name, which is already drawn in the bar inside it, so the
`<img>` is marked decorative - an image labelled *Preview* inside a button labelled *holiday.jpg* would
be announced twice, once with the wrong noun.

**The three-dot control is drawn as a glyph, not as a disabled button.** The design draws it in the
name bar and the name truncates against it, so leaving it out would change the bar's layout; drawing it
as `<button disabled>` would nest a button inside the item's button, which is the invalid markup the
sibling layout was chosen to avoid. A non-interactive icon at the same size keeps the layout and claims
nothing. The change that adds the menu (screens 09/09b) turns it into a trigger and lifts the bar out
of the button at the same time - one restructure, done when there is something to restructure it for.

### D7. The drawer unfolds itself from an effect on the open image count, and the effect lives in the drawer

`useEffect(() => { if (count > 1) open(); }, [count])` in `Drawer`.

**On the count, not on the batch**, which is what makes the spec's last two scenarios fall out without
being written: re-opening files that are all already open does not change the count, so the effect does
not re-run and a folded drawer stays folded - the same rule `addFiles` already applies to
`currentIndex`. Adding a genuinely new image *does* re-run it, so a drawer the user folded away opens
again. That is the reference's behaviour, verified in its `Preview` rather than assumed: its effect is
keyed on `filesLength` and calls `setOpen(true)` on every change above one. It is worth naming as
deliberate rather than incidental, because the alternative reading - unfold only on the crossing from
one to two - is what a "self-opening drawer" usually means, and it is not what the reference does or
what this ships.

**In `Drawer`**, where the reference has it in `Preview`. There it sits beside the drop listener and
the preview effects, because that is where the Wails app's file plumbing accumulated; nothing about
unfolding a drawer is the canvas's business, and slice 2 already moved the drop listener to the canvas
on the same principle - the rule belongs with the thing it acts on.

### D8. The toaster and the canvas's pane labels subscribe to the drawer

`components/ui/sonner.tsx` reads `open` and offsets notices by the drawer's full height when it is
unfolded, by the folded header's height when it is not - the two numbers the reference pairs in
`useNotify`, for the same reason its comment gives: change one without the other and the notice either
overlaps the drawer or floats above nothing.

The reference reads the drawer with `getState()` at the moment a snackbar is enqueued, deliberately, so
that `App`, `Preview` and `SidebarEnhancements` do not all re-render on every drawer toggle just to
recompute a margin they are not displaying. That argument does not apply here: the offset is a prop of
one mounted `<Toaster />` rather than a value computed by a hook every screen calls, so subscribing
re-renders the toaster and nothing else. Reading it at enqueue time would instead mean every `toast()`
call site in the application passing a style, which is the rule-to-remember that `providers.tsx`
already exists to avoid.

A notice raised before the drawer is unfolded keeps the offset it was shown with. Sonner re-reads the
offset on each render, so an open notice moves with the drawer; if it did not, the failure mode is a
notice 128px lower than it might be for a few seconds, which is not worth a mechanism.

**The canvas's two pane labels ride the same way**, off the same store and the same constant:
`features/preview/PreviewImage.tsx` reads `open` and lifts each chip by `DRAWER_HEIGHT` while the
drawer is unfolded. This is the design project's own `chipBottom`, and it is what `PreviewImage`'s
own comment already left to "the slice that unfolds the drawer" - slice 2 could put the chip 10px
above the canvas's bottom edge precisely because nothing was ever drawn over it, and the drawer is
the thing that changes that. A chip that did not move would sit behind the strip for as long as it
was showing, which is the same failure as a notice behind it and has the same fix.

It arrives as a second half of D8 rather than as its own decision, and later than the rest of this
design - it was agreed during implementation - because it is not a second rule. It is this one with
a second reader, which is also why R4 below now names three things wanting the bottom of the window
rather than two.

### D9. The item size the virtualizer estimates is measured, not derived

A small `useResizeObserver` hook, ported from the reference, measures the scroll element and gives the
virtualizer `clientHeight - 2 * padding` as the square item's side.

The strip's height is this application's own constant, so the only unknown in that subtraction is the
horizontal scrollbar - which takes no height on macOS, where it overlays, and does take height on
Windows and on GTK. Deriving it would mean writing that platform difference into a number, which is the
same mistake slice 2's D9 records paying for once already: *"the position's units are not what its type
says"*, trusted from a type rather than read from the source.

Being slightly wrong here is survivable by construction, and that is the point of D3's spacer layout:
this number sizes the two spacers and nothing else, so an error shifts how far the strip scrolls, never
how the thumbnails are spaced or padded.

### D10. A file whose pixels cannot be served is an empty thumbnail with its name

The design draws a `#1f1f23` surface behind every thumbnail. The `<img>` sits over it, decorative and
unlabelled (D6), so a request the protocol refuses leaves the surface showing with the name bar and the
checkbox intact. The file is still in the strip, still pickable, still choosable as the current image -
and choosing it draws the same broken image on the canvas that slice 2 accepted for the same case.

Slice 2 said this strip is "where a per-file failure first has somewhere to be shown", and this is the
honest amount of showing available: the `<img>` reports that it failed and not why. The protocol does
distinguish an unknown identity from an unreadable file, so a change that wants to say which - a
warning badge, a tooltip, a toast - has a source of truth waiting for it. Inventing one now would be
designing an error state for a case that needs a file to be deleted mid-session to reach.

### D11. Three new catalogue keys, where the reference labels nothing

`drawer.showImages` and `drawer.hideImages` for the fold toggle's two states, and
`drawer.selectImage` - *"Select {{name}}"* - for a thumbnail's checkbox.

`DrawerHeader`'s own comment set this up: the toggle carries a `data-slot` and no accessible name,
because "a hardcoded English label would be exactly the untranslated string the catalogues exist to
prevent", and "giving it a name is a change to the catalogues, and belongs with the slice that makes it
do something". This is that slice.

The checkbox's name interpolates the file's own name because a strip of twenty otherwise announces
twenty identical checkboxes with nothing to tell them apart. The reference leaves both unlabelled; it
is MUI and it predates this project's rule that every string on screen comes from the thirteen
catalogues.

## Risks / Trade-offs

**R1. The window can now hold hundreds of files, and only the strip was designed for it.** The file
store holds every record, and `addFiles` is O(n + m) per batch, but the navbar, the canvas and the
sidebar each hold one photograph at a time and are indifferent. → The one place the number is felt is
the strip, which is the one place this slice windows. The store's own cost is a `Set` build per batch,
which the reference measured and left in place at 500 files.

**R2. `loading="lazy"` is a hint, and a webview may ignore it.** WebKitGTK is the least certain of the
three. → The failure is bounded by D3: an engine that ignores it fetches the mounted set, which is the
visible range plus an overscan of 4 - a dozen thumbnails at 384px, not five hundred. Nothing is
incorrect, and the strip is not the place a difference would be noticed first.

**R3. A `Set` in the store, in a codebase that has kept its state to arrays and scalars.** A writer
that mutates it instead of replacing it produces no error and no re-render, which is the quietest bug
this slice can ship. → Every writer is one line, they are together in one file, and the store's tests
cover the observable half: that picking one image re-renders that thumbnail and leaves the rest alone.

**R4. Notices, the pane labels and the strip all want the bottom of the window.** The two offsets in D8
are where those heights are related, and a future drawer whose height depended on its contents would
have to keep both of them in step. → The drawer's height is a constant in `lib/constants.ts` and the
body's is a constant beside it; the toaster and the pane labels each derive from those rather than
repeating them, which is the arrangement `DRAWER_BLEEDING` was extracted for in the first place. Two
readers is also the point at which a third would be worth a shared hook rather than a third
subscription.

**R5. Click-away folds the drawer, and Tauri's drag-and-drop is not a DOM event.** A drop over the
canvas with the drawer open does not produce a click, so the drawer stays open over the canvas it just
accepted images onto - and then unfolds itself anyway, by D7. → That is the intended sequence rather
than a conflict: images arrived, the strip is what has something new to say about them.

**R6. Validated on macOS only.** The scrollbar (D9) and the lazy-loading hint (R2) are the two
platform-shaped pieces, and both are measured or bounded rather than assumed. → As every GUI slice in
this series has recorded: Windows and Linux are unverified on a machine.

## Open Questions

**Where the drawer's zoom lives** - slice 4's, not this one's. D1 keeps it out of the drawer store on
the argument that it is a property of how an image is looked at rather than of whether a strip is
showing, but the slice that builds the per-image transform is the one that can settle it, and nothing
here depends on the answer.
