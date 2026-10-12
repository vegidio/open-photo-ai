## Why

The window can open photographs and draw one of them, and there is still no way to reach the second.
The drawer that exists to answer that is folded shut, its fold toggle is disabled, its body is an
empty coloured rectangle, and Select all sits beside it greyed out. A user who drops twenty images on
the canvas sees the first one and has no evidence the other nineteen were opened at all.

This slice fills the drawer: the strip of thumbnails, the checkboxes the export queue will later read,
and the toggle that opens it.

## What Changes

- **The drawer unfolds.** Its fold toggle becomes live once an image is open, the body slides over the
  canvas without resizing it, and the drawer opens itself the moment a second image lands - which is
  the reference implementation's behaviour, and the answer to a window that has just accepted twenty
  files and shows one. Clicking away from an open drawer folds it again, as the reference does.
- **The drawer's body is the strip of open images.** One square thumbnail per open file, in the order
  they were opened, scrolling horizontally when there are more than fit.
- **Clicking a thumbnail makes that image the current one**, which is the whole of what "switching
  images" means: the canvas, the navbar's name and dimensions and the sidebar's miniature already read
  the current image and follow it. The current file's thumbnail carries the accent outline and its
  name bar is drawn inverted, so which one the window is showing is legible from the strip.
- **Every thumbnail carries a checkbox and a name bar.** The checkbox is the per-file selection the
  export queue reads in a later change; the name bar is the file's own name, truncated.
- **Select all becomes live**, with the three states the design draws - none, some (indeterminate),
  all - and a label that reads Unselect all once everything is picked. Opening images into an empty
  window selects the first of them, as the reference does.
- **A large batch stays cheap.** The strip mounts only the thumbnails near the visible range, with
  spacers standing in for the rest, and no thumbnail's pixels are requested until it is close to being
  seen. Five hundred dropped files are five hundred rows in a store and roughly a dozen mounted
  thumbnails.
- **Notices and the canvas's pane labels clear the open drawer.** The drawer is drawn over the canvas
  rather than beside it, so both of the things that sit at the canvas's bottom edge ride above
  whichever of the drawer's two states is showing rather than behind it.
- No file options menu, no closing an image, no zoom. The three-dot control the design draws on each
  name bar is drawn unavailable, for the reason given under Impact.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-shell`: gains what the drawer does once it has images - folding and unfolding under the user's
  control and on its own when a second image arrives, the strip of thumbnails it unfolds onto, what
  each thumbnail shows and which one is marked as current, and the select-all control's three states.
  Its existing requirement that these controls are unavailable with no image open is unchanged and
  stays true word for word.
- `gui-images`: gains the window's per-file selection - which images are picked, what picking all or
  none of them does, and the rule that opening images into an empty window picks the first - and the
  rule that the user can choose which open image is the current one.

## Impact

**Crates: `gui` only, and within it the React frontend alone.** No Rust changes: the thumbnails are
served by the URI scheme slice 1 built, at a bound the URL already carries, and nothing else here
crosses the IPC boundary.

Nothing in this slice is shared logic trapped in a front end. Which image the window is showing, which
ones are ticked and how many thumbnails are mounted are facts about a webview; the terminal front end
has no drawer and no selection.

**`rust-sak`: nothing is needed and nothing is missing.** This slice decodes nothing, hashes nothing
and reads no files; every thumbnail is a URL resolved by `opai` through the protocol handler, which
reaches `rust-sak` exactly as it already did.

**New dependency: one npm package, `@tanstack/react-virtual`** - the windowing the design's screen 08b
names by name and the reference already uses for this strip. design.md, D3, says why the alternatives
were refused. No new crates.

**i18n: three new keys** - the two states of the fold toggle and the per-thumbnail checkbox's
accessible name - added to all thirteen locales. `drawer.selectAll`, `drawer.unselectAll` and
`common.previewAlt` are already in the catalogue.

**Deliberate divergences from the Go application**, each argued in design.md:

- **A thumbnail's pixels arrive as a URL the webview loads lazily**, rather than the reference's
  `IntersectionObserver` around an IPC call that returns bytes. The deferral is the same and its
  trigger is the same; the mechanism is one the platform already has because slice 1 replaced the
  byte channel with a protocol.
- **The thumbnail is requested at a larger bound than the reference's 100.** The design crops each
  thumbnail to a square, and 100 is below what the square needs on a high-density display, so the
  reference's number would be a visibly soft thumbnail here. D4 has the arithmetic.

**Not in this slice: the file options menu** the design draws as a three-dot control on each name bar
(screens 09 and 09b). It closes an image, closes all of them, and reveals a file in the platform's
file manager - a new Rust command, a vendored dropdown menu, two store operations that can empty the
window, and the same menu again beside the navbar's filename. It is its own screen and its own change,
and folding it in here would put this slice well past fifteen tasks. The control is drawn in its place
and disabled, which is how this frame already draws every control whose effect is a slice away.

**Estimated task count: 13.** This is slice 3 of the four-slice `add-gui-images-*` series; design.md
carries the whole list, and only this slice is proposed here.
