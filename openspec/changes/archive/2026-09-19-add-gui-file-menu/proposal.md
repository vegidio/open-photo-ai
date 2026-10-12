## Why

Every photograph this application opens stays open for the life of the session. There is no way to
close one, no way to clear the window, and no way to get from a thumbnail to the file behind it on
disk. A user who drops the wrong folder has one remedy, which is to quit.

The design has drawn the answer since screen 09: a three-dot menu on each drawer thumbnail and beside
the navbar's filename, carrying *Close image*, *Close all images* and *Show in Finder*. Both triggers
already exist in the frame - `DrawerItem` draws the glyph as an inert icon and says in its own comment
that "the change that adds the file options menu turns it into a trigger", and `Navbar`, `files.ts`
and `transform.ts` each carry the same forward reference. This is that change.

## What Changes

- **The three-dot control on each drawer thumbnail becomes a menu**, and the navbar's filename gains
  the one the design draws beside it. Same three items in both, and the same component: the two
  differ only in where the menu is anchored.
- **Closing an image removes it from the window.** It leaves the strip, the selection and its
  remembered zoom and position. If it was the image being drawn, the canvas moves to the one that
  followed it rather than back to the first, which is the reference implementation's rule.
- **Closing all images empties the window**, returning it to the empty state the frame already
  draws - and folds the drawer, which is what screen 01 shows and what the reference does when its
  strip empties.
- **Show in Finder opens the platform's own file manager with the file selected**, labelled per
  platform - Finder, Explorer, or File Manager elsewhere. A reveal that fails says so; a menu item
  that quietly does nothing is worse than the failure it hides.
- **The file the window asks to reveal is one the application opened.** The registry that already
  decides which photographs may be served also decides which may be revealed, so a path the user
  never opened is refused rather than handed to the file manager.
- **The menu belongs to the drawer it opens from.** The drawer folds when the user reaches past it,
  and a menu rising out of a thumbnail is not reaching past it - operating the menu leaves the drawer
  open.
- Not here: nothing about export, crop or enhancements, and no second look at what the window is
  drawing. Closing an image changes what the window holds, not how it draws what remains.

## Capabilities

### New Capabilities

None. Both halves of this change extend capabilities that already exist: what the window holds and
serves, and what its frame draws.

### Modified Capabilities

- `gui-images`: gains closing - one image or all of them - and what closing does to the current image,
  the selection and the per-image zoom. Gains revealing an opened file in the platform's file manager,
  and extends the existing rule that only files the user has opened can be reached: it covers the
  reveal as well as the served pixels, and it now holds for a file the application could not measure.
  The existing requirements on serving pixels are unchanged.
- `gui-shell`: gains the file options menu on a drawer thumbnail and beside the navbar's filename -
  what it offers, where each of the two is anchored, and that operating one does not fold the drawer.
  Its existing rule that clicking away folds the drawer is narrowed by that last point rather than
  replaced, and the drawer now also folds when the last image is closed.

## Impact

**Crates: `gui` only.** The React frontend for the menu, the two triggers and the store operations
behind them; the Rust side for one new command and for moving the reveal out of `src/logs.rs`.

Nothing here is shared logic trapped in a front end. Which photographs a window holds, which of them
it is drawing and what a menu anchored to a thumbnail offers are facts about a webview - the terminal
front end has no session of open files, no drawer and no file manager to open. The reveal itself is
not this project's code at all; it is `tauri-plugin-opener`'s, already a dependency.

**`rust-sak`: nothing is needed and nothing is missing.** This change hashes nothing, fetches nothing,
extracts nothing, decodes no image and reads no file. The one filesystem-adjacent thing it does -
opening a file manager with an item selected - is the plugin's three platform implementations, which
`rust-sak` does not cover and has no reason to.

**A promotion, and it comes first.** `src/logs.rs` holds the reveal's thread-picking `cfg` - the macOS
main-thread requirement (FB22340592), the Linux zbus panic (plugins-workspace#3552) and Windows COM -
written for one caller. This change is the second caller, so per the module-layout rule that code
moves to its own module *before* the new command is written, rather than being imported across a
boundary or copied.

**New dependency: one vendored shadcn component, `dropdown-menu`.** Its Radix dependency is already
present through the unified `radix-ui` package. No new npm packages and no new crates.

**i18n: one new key.** The three menu items need none - `menu.file.close`, `menu.file.closeAll`,
`menu.file.showIn` with its `_darwin` and `_windows` contexts, and `errors.revealFileFailed` are
already in all thirteen catalogues. The two *triggers* are what this missed: each is a bare glyph, so
each needs an accessible name, and this project's rule is that every string on screen comes from the
catalogue. One key serves both - `menu.file.options`, "File options for {{name}}", interpolated with
the file's own name so that a strip of twenty does not announce twenty identical controls, which is
the argument `drawer.selectImage` already makes for the checkbox beside it. Added to all thirteen.

**Deliberate divergences from the Go application**, each argued in design.md:

- **The drawer's menu is anchored to the corner of its trigger, not to its centre.** The reference
  anchors that menu to the icon's centre; the design puts the menu's bottom-left corner off the
  icon's top-right. The navbar's anchoring matches the reference exactly.
- **A reveal names a file the application opened, and is refused otherwise.** The reference's
  `RevealInFileManager` takes any path the webview cares to send. This application already refuses to
  serve pixels for a file it never opened, and there is no reason for the file manager to be the one
  door that rule does not cover.

**A known gap, left open deliberately.** Closing an image makes the window forget it; the Rust side
keeps it in the opened-file registry and keeps whatever renditions are cached for it. Nothing
user-visible follows from that today - the renditions evict themselves, and re-opening the file
re-admits it - and withdrawing it correctly is more work than the rest of this change put together.
The first change that has a reason to need it should take it, rather than it being done here on
principle.

**Estimated task count: 13.** This is a change of its own, not a continuation of the
`add-gui-images-*` series, which closed on slice 4.
