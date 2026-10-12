## Why

The preview canvas accepts dropped images and says nothing while one is being dragged over it. The
window says images can be dropped - the empty canvas states it in words - but between picking a file up
and letting it go, nothing confirms that the pointer is over a region that will take it. A drop let go
of over the sidebar, the navbar or the folded drawer is silently ignored, by design, and the user's only
way to learn where the boundary is is to miss it.

`hooks/useDroppedImages.ts` closes by naming this gap exactly:

> `enter`, `over` and `leave` are ignored. Neither the design nor the reference draws a drag-over state,
> so highlighting one would be a new design decision made in passing.

This change makes that decision on purpose. While a drag is over the canvas, the region that will accept
it draws itself: a rounded rectangle carrying two beams of light travelling around its border - the
[magicui.design/docs/components/border-beam](https://magicui.design/docs/components/border-beam) effect,
ported to CSS.

## What Changes

- **A drag over the preview canvas draws the drop area.** A rounded rectangle appears with beams
  running around it, and disappears when the drag leaves the canvas or is let go of. Nothing is
  drawn under them: a `--border` track was tried and read as a grey box around the window.
- **It appears for any drag over the canvas**, without inspecting what is being dragged. The beam says
  *this is the region that takes a file*, not *these files are acceptable* - the second question is
  already answered on drop, by name, in the user's own language.
- **It is drawn wherever a drop is accepted**, which is the canvas in both its states: with a photograph
  open as well as on the empty canvas. What the window offers and what it accepts stay the same
  rectangle, which is the rule the existing hit-test already enforces.
- **Four beams, a quarter of a cycle apart**, each a fixed 384px long, alternating between the
  two colours the design gives the Autopilot analysing spinner - `#4ade80`, already `--success-bright`, and `#22d3ee`, which becomes a
  new token. One colour per beam, so the set reads the way the spinner's arcs do. Four rather than two
  because the canvas's perimeter is long enough that two leave most of it dark at any moment.
- **The rectangle is the canvas's own border**, traced rather than inset, and rounded at 12px like
  everything else the application draws over the canvas. An inset rectangle was tried first, on the
  reasoning that the canvas's left, right and top edges are the window's and a beam along them would
  read as window chrome. In the window it read as a second rectangle floating inside the canvas
  instead, which is not the region that accepts anything - so the mark is the region's edges. design.md,
  D5, carries the reversal, and the measurement that settled how long a beam may be before its corners
  stop looking like corners.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-images`: gains what the window does while a drag is in progress rather than only when it ends -
  that the rectangle which takes a drop identifies itself while the drag is over it, and that a drag
  elsewhere over the window still draws nothing.

  Its existing requirement that images dropped onto the preview canvas are opened is **modified** rather
  than left alone, in one respect: the region that accepts a drop now ends at an unfolded drawer's top
  edge rather than at the canvas's own. That is a real change to what is accepted - the drawer covers
  the lower part of the canvas rather than shortening it, so until now a drop let go of on the strip of
  thumbnails was taken. It is the question `useDroppedImages` parked in prose for whichever slice let
  the drawer unfold over the canvas, and this is that slice, because the mark cannot be drawn on a
  rectangle the drop does not actually use. The rest of the requirement - that a drop outside the region
  is ignored without notice, and that dropping and picking are indistinguishable - stays true word for
  word.

## Impact

**Crates: `gui` only, and the React frontend alone within it.** No Tauri command, no IPC, no Rust. The
drag events are already delivered to the frontend by `getCurrentWebview().onDragDropEvent`; this change
stops discarding three of the four kinds.

Nothing here is shared logic trapped in a front end. Whether a rectangle in a webview is currently lit
is a fact about a window, and the terminal front end has no drag to respond to.

**`rust-sak`: nothing is needed and nothing is missing.** No file is read, hashed, downloaded or
decoded. The dragged paths are not even examined.

**New npm packages: none, deliberately.** MagicUI's `border-beam` registry entry declares `motion` as a
dependency and uses it for one thing: animating `offsetDistance` from one percentage to another,
linearly, on infinite repeat. That is a CSS keyframe. A second beam is a negative `animation-delay`.
Taking `motion` on for this would be the frontend's first animation runtime, bought for a property CSS
already animates - design.md, D2, carries the full argument.

**One risk this change carries rather than resolves.** Both the upstream component and the CSS port
depend on `offset-path: rect(0 auto auto 0 round Npx)`, and `rect()` in `offset-path` is recent in
WebKit. macOS is the platform this series builds on and its webview is WKWebView, tied to the OS Safari
version; where it does not resolve, the beam is a static shape in a corner rather than a travelling one.
The change therefore verified it in the real application before the effect was built around it, and
`offset-path` is the route taken: on macOS 26.6 a shape travelled 281.7px along the top edge in 900ms,
and WebKit serializes the declaration back as the same path. Design.md, D3, carries the measurement and
keeps the fallback recorded for the platforms this series does not test - a rotating conic-gradient
mask, which reaches the same picture with no `offset-path` at all, though only on a shape close to
square. Windows and Linux are untested, as every GUI slice in this series has recorded.

**One new design token.** `#22d3ee` exists nowhere in the codebase today; `style.css` states the rule
twice, that every colour here is a token rather than a literal at the call site - *"it would have been
the first"*. It is added as `--activity`, named for what it and `--success-bright` mean together where
they appear side by side: the application is working, or ready to. Not `--info`, which would claim a
status neither the spinner nor the beam signals.

**This change is the token's first consumer, and the spinner is its second.** The Autopilot analysing
row the colours come from is not built - only its `sidebar.analysing` string is in the catalogue - so
the usual order is reversed here: the beam borrows a pair from a screen that does not exist yet. That is
recorded rather than avoided, because the alternative is inventing a second pair for the same meaning.

**i18n: none.** The effect carries no text. It is decorative and marked `aria-hidden`; what the drop
area means is already stated in words by the empty canvas, and a drag in progress is not something a
screen reader user is performing.

**No divergence from the Go application**, which has none of this: the Wails app ignores drag-over
entirely, exactly as this frontend has until now.

**Not in the design.** The screens file draws no drag-over state, and this change does not wait for one
- the requirement originates from the request that prompted it, and the spec says so, so a later reader
does not go looking for the screen it came from. The design may be retro-fitted later; nothing here
depends on that.

**Estimated task count: 8.** Well inside the limit; this is a single change, not a slice of a series.
