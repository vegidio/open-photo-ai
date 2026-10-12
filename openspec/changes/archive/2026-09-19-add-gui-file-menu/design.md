## Context

See proposal.md - Why. What matters for the approach is what is already in place.

**The two triggers are already drawn.** `DrawerItem` renders the three-dot glyph inside the
thumbnail's name bar as an inert `<Ellipsis>`, and its own comment names this change as the one that
turns it into a trigger. The navbar draws no control at all yet and needs one added.

**The reveal is already written, for one caller.** `crates/gui/src/logs.rs` holds the whole of the
platform discipline - the macOS main-thread requirement (Apple FB22340592, where an Objective-C
exception off the main thread aborts the process), the Linux zbus panic on a runtime worker
(tauri-apps/plugins-workspace#3552) and the Windows COM apartment - around
`tauri_plugin_opener::reveal_item_in_dir`. It is the plugin's *Rust API* rather than its command,
for exactly those reasons. That research is not to be repeated or copied.

**Nothing removes a file.** `useFileStore` has no remover, and `useTransformStore` none either; both
say so in their own documentation and both name this change. `Opened`, the Rust-side registry behind
the `opai://` scheme, has `admit` and `resolve` and no withdrawal.

**The drawer folds on any outside press.** `Drawer` registers a document `pointerdown` listener while
open and folds when the target is outside its own element. A menu that portals to `document.body` is
outside that element, which is the central interaction problem this design has to solve.

## Slices

**This is one change, not a slice of a series.** The `add-gui-images-1-ipc` .. `add-gui-images-4-zoom`
series closed on slice 4, which said so; `add-gui-canvas-background` and `add-gui-drop-beam` followed
it as standalone changes and this is a third. Screens 09 and 09b are one screen's worth of work and
fit inside the fifteen-task budget - see tasks.md - so there is nothing to divide.

`add-gui-images-3-drawer`'s proposal forecast this change by name and deferred the three-dot control
to it. That forecast is discharged here.

**Crates: `gui` only.** Within it, the React frontend carries the menu, the two triggers and the
store operations; Rust carries one new command and the promotion in D2. Nothing is shared logic
trapped in a front end: a session of open photographs, a drawer and a menu anchored to a thumbnail
are facts about a webview, and the terminal front end has none of them.

**rust-sak: nothing used, nothing missing.** No hashing, fetching, extraction, decoding or caching
happens here. The one filesystem-adjacent act is opening a file manager with an item selected, which
is three platform implementations `tauri-plugin-opener` already owns and which `rust-sak` has no
reason to cover.

**New crates: none.** `tauri-plugin-opener` is already a dependency. **New npm packages: none** - the
vendored shadcn `dropdown-menu` component sits on the unified `radix-ui` package the project already
has.

**Platforms.** Written for all three, validated on macOS. The reveal's Linux and Windows arms stay
unexercised, which is inherited rather than introduced - see R4.

**Execution providers: none are exercised.** Nothing here builds a session or runs a model.

## Goals / Non-Goals

**Goals:**

- One menu component serving both anchorings, so the two can never drift apart in content.
- Closing leaves no orphaned per-image state in the frontend.
- The reveal reaches the file manager without weakening the entitlement `Opened` establishes.
- The promotion in D2 lands *before* its second caller, per the module-layout rule.

**Non-Goals:**

- Withdrawing a closed file on the Rust side. D8 records why, and the spec deliberately says nothing
  about it.
- Any change to how an open image is drawn, magnified or compared. Closing changes what the window
  holds, not how it draws what remains.
- A confirmation step before closing. Neither the design nor the reference has one, and the action is
  cheap to undo by re-opening the file.

## Decisions

### D1. The reveal names a path the application validates, not an identity

`reveal_image` takes a path and refuses one the application never opened. `Opened` gains a set of
admitted paths alongside its identity map, and the command consults it before calling through.

**Why not by identity**, which would have reused `Opened::resolve` unchanged and cost nothing: a
record's identity is optional, and it is absent exactly when the file's bytes could not be read.
Those are still files the user opened and still rows in the strip - and they are the ones a user is
most likely to want to look at on disk, because something about them is wrong. Identity-addressing
would have forced the menu item to be greyed out for precisely that case.

**Why not an unvalidated path**, which is what the reference's `RevealInFileManager` takes: the
`opai://` scheme was built so that a location in a request names nothing and the interface can reach
only the photographs the user opened. A second door that accepts any path on the machine would make
that first door's care pointless.

The cost is a second collection on `Opened` whose growth is unbounded within a session, which is the
same shape as the identity map beside it and is bounded by how many files a user opens. See R6.

### D2. The reveal's thread discipline moves to its own module, first

`reveal()` and the error it produces move out of `src/logs.rs` into a module of their own, and
`reveal_log` then calls it as the first of two callers rather than as its owner.

This is the module-layout rule applied literally: a second caller promotes the file, the promotion
happens in the change that introduces the caller, and it happens *before* that caller is written, so
`reveal_image` is built against the shared location from its first line. Importing across into
`logs.rs`, or copying the three `cfg` arms, are the two things that rule names as never being the
answer.

The error moves with it and loses its subject. `LogsError::RevealLog`'s message reads "the log file
could not be shown in the file manager"; the shared error states the failure without naming what
failed, and each caller supplies its own subject. The `LogPath` variant stays in `logs.rs`, because
resolving where the log lives is not part of revealing anything.

### D3. The menu's content is portalled into the drawer

`DropdownMenuPortal container={drawerRef}` for the thumbnail's menu, with the ref `Drawer` already
holds threaded down through `DrawerBody` to `DrawerItem`.

The click-away check is `ref.current.contains(event.target)`. Portalling into that same element makes
the containment true rather than special-cased, which is the only version of this that keeps working
when the next portal-based control lands in the strip. The drawer's root sets no `overflow-hidden`,
so content rising above its top edge is drawn rather than clipped - which is why the drawer root and
not the strip is the container; the strip does clip, horizontally.

**Alternatives.** Excluding `[data-slot=dropdown-menu-content]` in the handler is one line, but makes
the drawer know that menus exist and has to be repeated per control. Suppressing the click-away while
a menu is open needs shared open-state between an item and the drawer for one interaction. Radix's
`modal` adds a body scroll-lock and `pointer-events: none` across the app, and does not even solve
it - the content is still portalled, so the document listener still fires.

**`modal` is in fact turned off**, which is a separate finding about the same prop. Left at Radix's
default of `true`, the pointer events it disables outside the open menu mean a press on a *second*
thumbnail's control never reaches it: the first menu stays up and the second never opens. Non-modal,
one press dismisses the first and opens the second, which is how a row of controls is expected to
behave. It also stops Radix marking the drawer `aria-hidden` while the drawer's own menu is open. The reference implementation
hit this same problem and wrapped its menu in a `stopPropagation` div with two accessibility lint
suppressions; that is the outcome this decision exists to avoid.

### D4. The thumbnail's name bar lifts out of the item's button

The item becomes a button covering the square with the image in it, and a sibling name bar holding
the file name and a real trigger button - the arrangement `DrawerItem`'s own comment already planned.

**The item button loses its accessible name and must be given one back.** Today the `<img>` is
`alt=""` on purpose and the bar's text inside the button *is* the button's name; lift the bar out and
the button is unnamed. It gets an explicit `aria-label` of the file's name, which says the same thing
the markup used to say implicitly. This is the one part of the restructure that is silent if missed,
so it gets its own assertion.

**Why not overlay the trigger** as a third absolutely-positioned sibling, the pattern D6 of the
drawer slice chose for the checkbox, which would have preserved the accessible name for free: it
decouples the trigger's box from the glyph's, and D5 depends on those being the same number. An
overlay trigger sized for a comfortable hit area would silently break the anchoring.

Nesting a button inside a button is not an option in either direction; avoiding it is why the item's
checkbox is a sibling at all.

### D5. Two anchorings, one component, and one constant behind the offset

| | Navbar (09b) | Thumbnail (09) |
|---|---|---|
| Relationship | menu top-centre on control bottom-centre | menu bottom-left on control top-right |
| `side` | `bottom` | `top` |
| `align` | `center` | `start` |
| `sideOffset` | twice the shared gap | the shared gap |
| `alignOffset` | - | glyph width |
| left margin | - | the shared gap |

**Two rows of this table are what measuring the running window changed**, and each is argued below:
the navbar's `sideOffset` is twice the gap rather than the gap, because half of it is spent inside the
bar; and the thumbnail's gap is a margin of its own rather than part of its `alignOffset`, because
`alignOffset` cannot express it.

The navbar's is native: Radix's `align="center"` on `side="bottom"` means exactly that relationship,
and it matches the reference implementation's anchoring for the same menu.

**Its `sideOffset` is twice the gap, and the second half is spent inside the navbar.** The offset is
measured from the trigger's own box, and that box is a 15px control centred in a 48px bar - so there
are already (48 - 15) / 2 = 16.5px between the trigger's bottom edge and the bar's, and one gap's
worth of offset put the menu flush against the bar it dropped out of. Measured: at 32 the menu's top
lands 15px clear of the bar, which is the edge a reader's eye measures from. The thumbnail's
anchoring has no equivalent - its trigger sits at the very bottom of the thumbnail, with nothing in
between.

The thumbnail's is not expressible with `align` alone. Radix offers `start` (leading edges flush),
`center` and `end` (trailing edges flush); what the design asks for is the menu's leading edge at the
control's *trailing* edge. `align="start"` with an `alignOffset` of the control's own width shifts it
exactly that far. **That is only true while the trigger is exactly the glyph's box**, which is what
D4 secures and what R1 is about - so the glyph's size is a named constant and the offset is written
in terms of it.

**The gap on top of that is NOT expressible with `alignOffset`, which measuring the window
established.** Radix positions through floating-ui's `shift` with a `limitShift` limiter, which
clamps a cross-axis offset at the point where the floating element would stop overlapping its anchor:
a menu is not allowed to float free of the control that opened it. The trigger is 15px wide, so 15px
is the whole of what an `alignOffset` can move this menu - `alignOffset: 31` moved it 15 and left the
two corners touching, which is what the window drew and what the author saw.

So the offset buys the flush relationship and **the gap is a left margin on the content**, applied
after positioning, where nothing clamps it: `alignOffset: FILE_MENU_GLYPH` plus a margin of
`FILE_MENU_GAP`. Measured on the first and last thumbnail of a scrolled strip: `GAPx=16.0 GAPy=16.0`
at both.

The gap is 16px, which is what the mockup draws: it places the menu at `(+16, +16)` from the
control's corner, consistent in both axes and so intentional rather than eyeballed. Confirmed with
the author. The corners therefore do not meet; the menu clears the glyph diagonally.

`alignOffset`'s sign is direction-dependent and should be confirmed against the first render rather
than trusted from the documentation.

This is a **deliberate divergence from the reference**, which anchors the thumbnail's menu to the
icon's centre (`anchorOrigin: {vertical: 'center', horizontal: 'center'}`). The design draws the
corner relationship instead, and the design is the authority for this application's appearance. The
navbar's anchoring is unchanged from the reference.

### D6. One menu component, parameterised by anchoring, not by content

A single component takes the file it acts on and the anchoring props, and both call sites render it.
The three items, their order, the separator's position and the platform-dependent label are written
once. The reference reached the same conclusion with its shared `MenuFileOptions`, and it is the only
thing keeping the navbar's copy from drifting from the drawer's.

The platform label uses i18next's `context` mechanism - `menu.file.showIn` with `_darwin` and
`_windows` variants already in all thirteen catalogues - rather than interpolating a file manager's
name into a sentence. Word order around a product name is not universal, so the whole sentence has to
be one translatable unit. An undefined context falls back to the base key, so there is no third
variant to keep in sync.

`isMacOs()` and `isWindows()` in `ipc/os.ts` already answer the platform synchronously, which is what
lets the label be correct on first paint.

### D7. Closing is two store operations, and it clears everything keyed by the file

`closeFile(path)` and `closeAll()` on the file store, with the transform store's entry removed
alongside - the removal its own documentation already named and deferred.

The index fixup is the reference's, and its middle rule is the non-obvious one:

```
   close at removedIndex, currentIndex = c

   files empty afterwards          -> c = 0
   removedIndex < c                -> c - 1      (indices below shifted down)
   c >= files.length afterwards    -> length - 1 (the last one was current)
   otherwise                       -> c unchanged
```

`c` unchanged is what makes closing the current image move *forward*: the slot keeps its number and
the photograph that followed has slid into it. Closing repeatedly walks through the open set rather
than snapping back to the first, which is what the spec's scenarios pin.

The transform store is keyed by identity and the file store by path, so closing has to resolve one
from the other before it removes the record. A file with no identity has no transform entry to
remove, which is consistent - it was never drawn.

**This reverses which store imports which, and one accessor moves out of the way.** The file store is
now the one that knows a photograph has gone, so it is the one that calls into the transform store -
where until now the traffic ran the other way, through `useCurrentTransform`, the transform store's
own accessor reading the *file* store for the current identity. Left where it was, that import closes
a cycle. It moves to `hooks/useCurrentTransform.ts`: it is a composition of the two stores rather
than a member of either, so neither store owns it, and it sits with the other hooks that read across
regions. Nothing about it changes but where it lives.

The reference's `clearAll` also drops decoded-pixel and face caches. Neither exists here: the pixels
live behind a URL rather than an object URL, which is what slice 1 was for, and faces are a later
change. There is nothing else to clear.

### D8. The Rust side keeps a closed file, deliberately

`Opened` does not withdraw and the rendition cache does not evict. The frontend forgets the file and
therefore never asks for it again.

Withdrawal is defensible - `Opened` is documented as the reachability set, and a closed photograph is
not open. It is deferred because nothing user-visible follows from it today: renditions evict
themselves under the LRU, re-opening re-admits for free, and an `<img>` request already in flight
when the menu fires would be cancelled mid-response by a withdrawal that happened correctly. Doing it
properly also means deciding what two paths hashing to the same bytes means for a withdrawal keyed by
identity, which is a question with no forcing case behind it yet.

Recorded as a gap in proposal.md so that the first change with a reason - an export that must not
serve a closed file, most likely - picks it up rather than rediscovering it. The specs say nothing
about it in either direction, which is deliberate: an untestable scenario is not a scenario.

## Risks / Trade-offs

**R1. The thumbnail's `alignOffset` silently drifts if the trigger gains padding.** The anchoring is
correct only while the trigger's width equals the glyph's. A later change adding a comfortable hit
area would move the menu sideways by however much it added, and nothing would fail. → The glyph size
is one named constant, referenced by both the icon and the offset, with the dependency stated where
the offset is written. A rendered-geometry assertion is not available under jsdom, so the constant is
the whole of the defence.

**R2. Portalling into the drawer changes Radix's collision boundary.** Content portalled into a
positioned, transformed ancestor - the drawer is both, since folding is a `translateY` - can have its
collision detection and its own transform resolved against that ancestor rather than the viewport. →
Verify against the first and last thumbnails in a scrolled strip, which are the two positions where a
menu would collide with a window edge.

**It happened, in a different way than predicted, and it is fixed.** The transform did not interfere.
What did is that Radix measured collisions against the *viewport*, while what actually clips a menu
is the shell's `relative min-w-0 flex-1 overflow-hidden` box - the drawer's own parent, narrower than
the window by the width of the sidebar. A menu opened from the last thumbnail of a scrolled strip was
positioned correctly and then cut in half at that box's edge. The fix is one prop: `collisionBoundary`
set to that parent, so the menu shifts left to stay visible. The parent rather than the drawer itself,
because the drawer is 176px tall and this menu is supposed to rise clear above it. D3's fallback was
not needed.

**R3. Closing an image unmounts the trigger the menu is anchored to.** Choosing *Close image* from a
thumbnail's menu destroys that thumbnail, and Radix returns focus to a trigger that no longer exists;
*Close all images* destroys the whole strip and the drawer folds under the open menu. → The menu must
close before the store write, not after, so the unmount happens with no open menu to return focus to.
This is the first thing to exercise by hand on both call sites.

**R4. The reveal's Linux and Windows arms are still unexercised.** `logs.rs` records that only the
macOS arm has ever run; promoting the code in D2 does not change that, and this change adds a second
caller to both untested arms. → Inherited rather than introduced, and stated here so it is not read
as newly verified. Both stay the first thing to check when this application is first built on those
platforms.

**R5. The accessible-name regression in D4 is invisible.** An unnamed button renders identically. →
Its own assertion, named for the property rather than for the markup.

**R6. `Opened`'s path set grows for the whole session.** D1 adds a collection that nothing removes
from, and D8 declines to remove from the one beside it. → Bounded by how many files a user opens in
one session, holding one path each; the identity map beside it already has this shape. Whichever
change takes D8's withdrawal takes this with it.

**Trade-off: the menu duplicates two actions in two places.** *Close all images* sits in the navbar's
menu and in every thumbnail's, so a strip of twenty offers it twenty-one times. That is the design's
call and the reference's, and the alternative - a different menu in each place - costs the user the
guarantee that the two controls do the same thing.
