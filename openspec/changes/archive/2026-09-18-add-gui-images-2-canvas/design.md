## Context

See proposal.md - Why. What shapes this slice is that the hard half is already built and the frontend
is the half that is not.

`gui` already has: `open_images`, which opens the picker and answers with records; `describe_images`,
which does the same for paths that arrived any other way; `ImageRecord` - path, identity, dimensions,
extension, size; the `opai` URI scheme, which serves an admitted image's pixels at a bound on its
longest edge and tells the webview it may keep them; and `frontend/ipc/images.ts`, whose
`renditionUrl(identity, bound)` is the whole of how a photograph is drawn. Nothing in the window calls
any of it.

The frontend is the four always-present regions of `gui-shell`, every image-acting control drawn and
disabled, and three stores - `setup`, `settings`, `enhancements` - the last of which is deliberately
the Wails store's *persisted* surface, key and all. There is no file store and no notion of a current
image; `App.tsx` carries `const hasFiles = false` with a comment saying the slice that can load a file
is the one that turns it into a store read. This is that slice.

The reference implementation reaches the same screen through `useFileStore`, `useImageStore`,
`useAppStore.previewMode` and a `Preview` effect that fetches the original, copies it into the enhanced
slot and then runs the enhancement stack over it. Its enhancement half has no counterpart here yet,
and the parts of it this slice needs are the parts that do not depend on it.

## Slices

The four slices that take the application from an empty canvas to an open, zoomable image. This is a
new series; the `add-gui-1-foundation` .. `add-gui-7-settings-dialog` series closed out on the Settings
pair and is not continued here.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-images-1-ipc` | `opai::image::resize`; the picker command and the describe-paths command; the image record; the identity-addressed rendition protocol and the opened-file registry behind it. No UI | - | **DONE** |
| 2 | `add-gui-images-2-canvas` | Browse images and drag-and-drop wired to slice 1; the file store; the canvas drawing the current image in all three preview modes; the navbar's filename and Dimensions; the sidebar miniature | 1 | **CURRENT** |
| 3 | `add-gui-images-3-drawer` | Screens 08 and 08b - the thumbnail strip, its windowing and deferred decode, the per-item checkbox and name bar, Select all, the current-file outline, switching images, and the fold toggle made live | 2 | pending |
| 4 | `add-gui-images-4-zoom` | The per-image transform shared across panes, the drawer's zoom slider and the wheel/pinch zoom, split mode's draggable handle, and the viewport rectangle drawn over the sidebar miniature | 3 | pending |

**Slice 2 is this change.** Slice 1's design left one thing for this slice to settle: whether the
sidebar miniature fits here or moves to slice 4, where the viewport rectangle drawn over it already
lives. **It stays here.** It is an `<img>` drawn from a URL this slice already builds, replacing a
line of text that is already conditional, and the count lands at 15 tasks with it included - at the
limit, not over it. Moving it would have left the sidebar saying "No preview available" beside a
canvas showing a photograph for two whole slices, which is a worse intermediate state than the one
task it saves is worth.

**Crates.** `gui` only, and within it almost entirely the React frontend - see proposal.md - Impact
for why none of this belongs in `opai`. The one Rust addition is D2.

**rust-sak: nothing used directly, nothing missing.** This slice decodes, encodes and hashes nothing;
every pixel it draws comes from the protocol handler slice 1 built, which reaches `rust-sak` through
`opai` exactly as it already did.

**New crates: none. New npm packages: none** - D5 and D6 are why the two the reference uses here are
not needed.

**Platforms.** Written for all three, validated on macOS. Two things are platform-shaped: the
rendition URL's form, which `convertFileSrc` already handles and slice 1 recorded as untested off
macOS, and the drag-and-drop event, which Tauri normalises across the three and which this slice
verifies on macOS only.

**Execution providers: none are exercised.** Nothing here builds a session or runs a model.

## Goals / Non-Goals

**Goals:**

- One path from a file the user named to pixels on the canvas, whichever of the two routes they used.
- A file store the next two slices extend rather than replace: the drawer's strip (slice 3) and the
  per-image transform (slice 4) are both keyed off what this store holds.
- The three comparison modes drawn as the design draws them, so slice 4 adds movement to a layout that
  is already right rather than a layout and movement at once.

**Non-Goals:**

- **Enhancement of any kind.** Both sides of a comparison show the same photograph, which is what the
  reference shows for a file with an empty stack. The pipeline behind the enhanced side is a much
  later slice.
- **The drawer's strip of thumbnails** (slice 3), and therefore **switching the current image**: this
  slice can open images and draw the first of them, and the only way to reach a second one is to open
  it. The store supports it; nothing in the window yet asks.
- **Zoom, pan, and dragging the split divider** (slice 4). The divider is drawn where the design draws
  it and does not move.
- **Closing an image** (screen 09), which is also what would empty the store; nothing in this slice
  removes a file.
- **A state for an image whose pixels cannot be served.** An admitted file that has been deleted
  between being described and being drawn is a broken image in the canvas. The reference does not
  handle it either - its `getImage` throws into an unhandled rejection - and inventing a state for it
  here would be designing the error screen for a case no user has yet been able to reach. Slice 3's
  thumbnails are where a per-file failure first has somewhere to be shown.
- **The Dimensions hover popover** the reference draws. It compares source, cropped and output sizes;
  with no crop and no scale in the application, all three are the same number and the popover would be
  a tooltip repeating the label under the pointer.

## Decisions

### D1. The drop is listened to by the frontend and funnelled through `describe_images`

`getCurrentWebview().onDragDropEvent` in the shell, filtering for the `drop` payload, which carries
the paths. Those paths are partitioned (D2), the supported ones are handed to `describeImages`, and
the records join the store by the same call the picker's do.

*Alternative: handle the drop in Rust*, where `WindowEvent::DragDrop` is also available and where the
extension list already lives, emitting records to the frontend as an event. It is what the reference
does, because Wails gives it no choice - its drop handler is Go, and the unsupported-file warning has
to be emitted to the frontend anyway *because Go has no i18n catalogue to phrase it with*. Doing it
that way here would leave `describe_images` - a command shipped one slice ago, specified, tested and
documented as the route for paths that arrive any other way - with no caller at all. The command was
built for this; this is the thing it was built for.

`enter`, `over` and `leave` are ignored. The design draws no drag-over state for the canvas and the
reference has none either, so a hover highlight would be a new design decision made in passing.

### D2. The frontend asks the decoder which extensions it opens, and partitions the drop itself

One new command answering `opai::image::input_extensions()`, and a filter in the frontend over the
dropped paths' extensions. What it refuses is named in the existing `toasts.unsupportedFiles` notice,
whose thirteen locales and plural forms are already in the catalogue.

The partition is by extension, as the reference's `PartitionSupportedFiles` does it, and the toast is
raised by the frontend for the same reason the reference raises it there: the message needs a plural
form that only the i18n catalogue gets right in languages with more than two.

*Alternative: no command, and judge a dropped file by whether the application could read it.*
`describe_images` already reports a file it cannot read as a record with no dimensions, so the
interface could drop anything on the floor that came back without them. It is tempting - it is
content-derived rather than name-derived, so a JPEG named `.txt` would work - and it is wrong twice
over. Describing reads **every byte** to compute the identity, so dropping a 4 GB video onto the
window would read 4 GB before concluding it is not a photograph, and a dropped folder would be hashed
rather than dismissed. Worse, a corrupt but genuinely supported photograph would be reported to the
user as a format the application does not open, which is a lie about the one case the spec's tolerance
was written for.

*Alternative: keep the extension list in the frontend.* `crates/opai/src/image/raw.rs` already answers
this, in the rule slice 1's D5 quotes: *"A second list would be a copy of someone else's truth: a
backend that gained a format would go on being refused by a stale list."* The command costs one
function and one `invoke`.

The picker route is not partitioned: the platform's dialog already applies the same list as a filter,
and re-checking what it returned would be checking the filter rather than the files.

### D3. The store holds records and an index, and is the shape slices 3 and 4 extend

`stores/files.ts`: the open images in order, the index of the current one, and one writer that adds a
batch. Adding de-duplicates by path within the batch and against what is already open, appends, and
moves the current index to the first newly added file - and leaves it alone when the batch added
nothing new. All four behaviours are the reference's `addFiles`, including the last, which is why
opening the same folder twice does not jump the canvas back to its first photograph.

Selection - the drawer's checkboxes, its Select all, and the `selectedPaths` set the reference keeps
beside the list purely to make a membership test O(1) - is **not** here. It is written in the slice
that draws the checkboxes, against the strip that reads it, rather than guessed at now.

The current image is read through one accessor, `useCurrentFile`, exactly as the reference does it,
so that four regions asking "which photograph" ask it one way.

`App.tsx`'s `const hasFiles = false` becomes a read of this store, which is what that line was left
there for. Every control that takes `disabled` from it keeps taking it; the ones whose behaviour is a
later slice - the fold toggle, Select all, the zoom slider, Add enhancement, Export - stay disabled
regardless, because a control that is enabled and does nothing is a worse answer than one that is
visibly unavailable. The comparison control is the one this slice turns on, and it is turned on
because this slice gives it something to do.

### D4. The preview mode is its own persisted store, under the Wails app's key

`stores/preview.ts`, holding the chosen comparison and nothing else, persisted by `zustand/persist`
under `app-storage` - the reference's `useAppStore` key and its default of side-by-side.

The key is deliberate and has a precedent in this codebase: `stores/enhancements.ts` persists under
`enhancements-storage` *"the Wails app's key, unchanged: a user upgrading from it keeps the setting
they chose."* The same argument applies here, and it costs nothing to honour.

Its own store rather than a field on the file store, because it is not about a file: it survives every
file being closed, and it survives the application being quit, which nothing else in the file store
does.

### D5. The canvas is `<img>` elements over the dotted surface, and the split is a `clip-path`

A pane is an `<img src={renditionUrl(identity)}>` centred in its half, letterboxed by `object-contain`.
Full draws one pane across the canvas; side by side draws two, each half; split draws the enhanced pane
absolutely over the original, revealed from the divider rightward by `inset(0 0 0 50%)`, with the
design's 2px rule and its circular grip drawn over the seam. The divider does not move until slice 4.

*Alternative: `react-compare-slider`, which the reference uses for split mode.* It brings the drag this
slice is not delivering, and the layout it produces is the one the design already specifies as a clip.
Slice 4 makes the divider draggable across panes that are *also* zooming and panning in step - a
constraint the library does not know about, since its two children move independently. Adding it now
would mean adopting a dependency this slice does not need and removing it in the one that would.

**Both panes point at the same URL.** The identity and the bound are the same, the protocol serves it
immutable, and the webview's cache means the second pane costs a cache lookup rather than a decode.
That is also why switching between the three modes is free, and why the sidebar miniature (D7) costs
nothing either.

### D6. The canvas asks for the image at its own dimensions, with no bound

`renditionUrl(identity)` - no `size` - which is the reference's `getImage(file, 0)` for the preview.

It is the expensive choice and it is the right one: the zoom in slice 4 goes to 8x, and a canvas fed a
canvas-sized rendition would be showing the user an eight-times-magnified thumbnail. Encoding a 100 MP
photograph at full size is the cost, paid once per image because the response is immutable and the
webview keeps it. R2 is what to do if that proves too much.

The bound exists for the thumbnails slice 3 draws, which is what slice 1's open question was about;
this slice answers only its own half of it.

### D7. The sidebar miniature draws the same URL the canvas does

Not a bounded rendition of its own. A 144px-tall panel could ask for `size=288` and get a few kilobytes
instead of a few megabytes - but it would be a **second decode and encode of the same photograph**,
where reusing the canvas's URL is a cache hit on bytes the webview already has. The panel is one
element on screen at a time, so the memory a browser spends scaling it down is bounded by the one
image; the drawer's strip, where twenty of them are on screen at once, is exactly where the bound earns
its keep, and that is slice 3.

The viewport rectangle the design draws over this miniature needs the pane's transform, which is
slice 4's.

### D8. The panes are labelled, which the reference does not do

An "Original" and an "Enhanced" chip, bottom left and bottom right of their panes, riding above the
drawer as the design places them. Two new catalogue keys in thirteen locales.

The reference draws two unlabelled images. With an empty enhancement stack - which is every image in
this slice, and the first seconds of every image for several slices after it - they are identical, and
an unlabelled pair of identical photographs reads as a bug. The design draws the chips; this follows
the design, and the chips keep meaning what they say once the pipeline lands behind them.

### D9. The drop lands on the canvas, which is the region that offers it

The reference marks its canvas as the toolkit's drop target. Tauri instead delivers a drop to the
window, as an event carrying a position, so the same rule is the listener's to enforce: the hook
takes a ref to the canvas element, converts the position into the DOM's own pixels, and hit-tests it
against that element's rectangle before it asks for anything.

**The position's units are not what its type says.** Tauri hands the frontend a `PhysicalPosition` on
every platform, and only Windows reports one: wry reads the position from `ScreenToClient`, whose
client coordinates are device pixels. macOS reads `draggingLocation()`, which AppKit gives in points,
and GTK reports logical units too; `tauri-runtime-wry` relabels the pair as physical either way
without scaling it. Trusting the type cost this slice a round: dividing by `devicePixelRatio` on a
Retina Mac halved every coordinate, which pulled the sidebar and the drawer inside the canvas's
rectangle and accepted exactly the drops the rule exists to refuse. `positionScale` therefore divides
on Windows and nowhere else, read out of wry 0.55's three `drag_drop.rs` files rather than out of the
type.

The canvas is what makes the offer - "Drag and drop images to start editing them" is drawn on it and
nowhere else - so it is the rectangle that takes the offer up. A drop over the navbar, the sidebar or
the folded drawer is ignored in silence: those regions never claimed to take a photograph, and a
notice about a drop they did not invite would be a notice about a gesture the user can simply repeat
two inches over. The drawer folds *over* the canvas rather than beside it, so the slice that lets it
unfold decides what a drop on the strip of thumbnails means; while it stays folded it is below the
canvas's rectangle and the rule needs nothing said about it.

The hook is called from `Preview` rather than from `App` for the same reason: the listener belongs
with the element it hit-tests against, and a ref threaded down from the shell would put the rule in
one file and the rectangle in another.

*Alternative: accept the drop anywhere on the window*, which is what this slice first built and what
its spec first stated. It is cheaper - no hit test, no ref, no coordinate conversion - and it never
refuses a photograph the user meant to open. It was dropped because the window is not one undivided
surface: the canvas says it takes images and the other three regions say other things, and a window
that quietly took a drop over its sidebar would be answering a gesture that was not aimed at it.

## Risks / Trade-offs

**R1. `dragDropEnabled` is on by default, and it takes the webview's own drag events with it.** Tauri
intercepts file drops natively, which is what delivers the event this slice listens to, and it also
means HTML5 `dragover`/`drop` handlers never fire for files. Nothing here wants them. → Recorded so
that the slice which one day wants a drag-over highlight looks for it in the Tauri event's `enter` and
`leave` payloads rather than in the DOM.

**R2. A full-resolution rendition of a very large photograph is megabytes of JPEG and hundreds of
megabytes decoded in the webview.** D6 accepts that deliberately, and the webview's cache - which slice
1 chose over the reference's 1.5 GB LRU - has no bound of its own. → It is one image per open
photograph rather than one per thumbnail, and the reference pays strictly more for the same picture,
base64-inflated through its IPC channel. If it proves too much, the canvas asking for a device-pixel
bound and slice 4's zoom asking for an unbounded one is a change to a URL, not to a component.

**R3. A file that goes away between being described and being drawn shows a broken image.** Named as a
non-goal above. → The window is refused the pixels, not given the wrong ones: the protocol answers
"unreadable" and the `<img>` fails. Slice 3 is where a per-file failure first has a place to be
reported.

**R4. The extension list crosses the IPC boundary once per launch and is then held in the frontend.**
A format the decoder gains mid-session would not be noticed - which cannot happen, because the list is
compiled in. → It is fetched where it is first needed and cached for the session.

**R5. Three of the frame's controls are enabled-looking neighbours of controls that stay disabled.**
With an image open, the comparison group lights up while the fold toggle, Select all and the zoom
slider beside it stay grey until slices 3 and 4. → That is the slicing this series committed to, and
the alternative - enabling a control whose effect does not exist - is the thing this project has
already argued against twice. Anyone running the build between slices sees an honest intermediate
state.

**R6. Drag-and-drop is verified on macOS only, and D9's hit test is platform detail.** → The event is
Tauri's own abstraction over three platforms, but the rule built on its position is not: the units
differ per platform (D9), and the hit test assumes the position and the element's rectangle share an
origin, which holds because `titleBarStyle: "Overlay"` puts the webview under the title bar rather
than below it. A platform that inset the webview inside the window would need that offset subtracted.
The Windows branch is what wry's source says and is covered by a test that states the units; neither
it nor Linux is verified on a machine, as every GUI slice in this series has recorded.

## Open Questions

**What bound the drawer's thumbnails ask for**, which slice 1 left open and this slice does not need to
answer: the canvas and the miniature both want the photograph at its own size (D6, D7). The strip is
where 100, 128, 144 or a device-pixel multiple of one of them has to be chosen, and it changes nothing
here - the bound is a number in a URL and the protocol treats every value alike.
