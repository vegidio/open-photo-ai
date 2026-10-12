## Context

See proposal.md - Why. What matters for the approach:

- **One hook already owns the drag channel.** `hooks/useDroppedImages.ts` registers
  `getCurrentWebview().onDragDropEvent`, converts the reported position into CSS pixels
  (`positionScale`, which exists because the three platforms disagree about the units), hit-tests it
  against the element the caller hands it (`isOver`), and returns nothing. It discards `enter`, `over`
  and `leave` and acts only on `drop`.
- **Tauri's `enter` is a *window* event, not an element one.** It fires once when the drag crosses into
  the window, wherever it crossed. A drag that enters over the navbar and moves down onto the canvas
  produces one `enter` (outside) followed by `over` events (inside) - so `enter` alone cannot drive the
  mark. See D4.
- **The canvas has no border and no radius.** `Preview.tsx` renders `absolute inset-x-0 top-0 bottom-12`,
  flush to the window's left, right and top edges.
- **The test harness for this already exists.** `useDroppedImages.test.tsx` mocks the webview module,
  captures the handler so a test can deliver any `DragDropEvent`, and stubs a `DOMRect` for the canvas
  because jsdom lays nothing out. Drag-over tests need no new scaffolding.
- **`#4ade80` is `--success-bright`; `#22d3ee` is nothing yet.** `style.css` says twice, in prose, that
  a colour here is a token rather than a literal at the call site.

## Goals / Non-Goals

**Goals:**

- One listener, one hit-test, one answer to "is the pointer over the region that accepts this" - shared
  by what is drawn and what is accepted, so they cannot come to disagree.
- No animation runtime added to the frontend.
- A beam component general enough that the crop dialog or an export target could reuse it, without
  being built for a second caller that does not exist.

**Non-Goals:**

- Inspecting the dragged payload. Settled: the mark appears for any drag.
- A drag-over state anywhere else in the window - the drawer, the sidebar, an export drop target.
- Matching MagicUI's prop surface exactly. It is a reference, not a contract.

## Slices

This is a single change, not a slice of a series. It depends on nothing and blocks nothing.

## Decisions

### D7 - The drawn region and the hit-tested region are one element, and the drawer shortens both

`Preview` is `absolute inset-x-0 top-0 bottom-12`: inset by the drawer's folded 48px header
permanently, because unfolding slides the drawer's 128px body up *over* the canvas rather than
resizing it - which is what makes folding cheap, since nothing re-lays-out and the preview image never
rescales. The consequence for this change is that the canvas's own bottom edge sits 128px *inside* an
unfolded drawer. Hit-testing against it accepts a drop let go of on the strip of thumbnails, and
drawing on it puts the bottom beam across them.

So the region is its own element, `inset-x-0 top-0` with its bottom set to the drawer's body height
while the drawer is open and to zero while it is folded, and **the hook hit-tests that element rather
than the canvas**. One rectangle answers both questions structurally, which is what the spec's "what
is offered and what is taken up stay the same answer" asks for; the element is always mounted, because
it is the hit test, and only the beams come and go with the drag.

This is the question `useDroppedImages` parked in prose - *"the slice that lets the drawer unfold over
the canvas inherits the question of what a drop on the thumbnails means; until then the drawer is
never over the canvas"*. The drawer now does unfold over it, so the question is answered here: the
thumbnails are not the canvas, and a drop on them opens nothing, exactly as the spec's scenario for a
drag over the image drawer already required.

### D1 - `useDroppedImages` reports the hover; no second listener

The hook's signature changes from `(target) => void` to `(target) => boolean`. It already holds
everything the answer needs - the channel, the scale correction, the rectangle - and the boolean is the
same `isOver(element, position)` call the drop path makes, evaluated earlier.

**Alternative rejected:** a separate `useDragOver(ref)` registering its own `onDragDropEvent`. Tauri
allows a second listener, but it would duplicate `positionScale` and `isOver`, and the two copies are
exactly what "what is drawn and what is accepted are the same rectangle" forbids. The spec makes that a
promise; one hit-test makes it structural.

The hook keeps its name. Opening dropped images is still what it is for; reporting the hover is a second
return from the same channel, not a second responsibility.

### D2 - CSS keyframes, not `motion`

MagicUI's `BorderBeam` uses `motion/react` for one thing:

```
initial={{ offsetDistance: "0%" }}
animate={{ offsetDistance: "100%" }}
transition={{ repeat: Infinity, ease: "linear", duration }}
```

An infinitely repeating linear interpolation of one property between two constants. That is
`animation: beam 6s linear infinite` with a two-stop `@keyframes`, and upstream's `delay: -delay` is
`animation-delay` with the sign already negative. Nothing else in the component is animated: the mask
trick, the `offset-path`, the gradient and the `aspect-square` are static CSS that Tailwind emits either
way.

Adopting `motion` would make it the frontend's first animation runtime, for a component that needs none
of what a runtime is for - no spring, no gesture, no interruption, no layout animation, no orchestration.
This frontend has repeatedly declined to take a dependency for the fraction of it that would be used;
`react-zoom-pan-pinch` was turned down in slice 4 on the same reasoning.

**Alternative rejected:** the Web Animations API (`element.animate`). Same effect, but it puts the
animation in an effect that has to be cancelled on unmount and re-created when a prop changes, where
CSS needs neither.

**Consequence:** the component's props are CSS custom properties rather than a `Transition` object. It
keeps `size`, `duration`, `delay`, `colorFrom`, `colorTo`, `reverse`, `initialOffset`, `borderWidth` and
`className`, written to project convention - `type` alias, arrow function, optional-property syntax - and
drops `transition`, which has no CSS counterpart worth inventing.

### D3 - `offset-path: rect()` is verified before the effect is built on it, and has a stated fallback

Both the upstream component and the CSS port move the beam by animating `offset-distance` along
`offset-path: rect(0 auto auto 0 round Npx)`. `rect()` in `offset-path` is recent in WebKit, and macOS is
the platform this series builds on - WKWebView tracks the OS Safari version. Where it does not resolve,
the beam does not fail loudly: it sits at the element's origin and never moves, which is a static blob in
a corner rather than an error anyone would see in a test.

So the first task is a check in the real application, not in jsdom and not from a support table: render
the shape, confirm the beam travels. Windows and Linux stay untested, as every GUI change in this series
has recorded.

**Verified, and `offset-path` is the route taken.** Checked in the running application on macOS 26.6
(WKWebView): a 40px square with `offset-path: rect(0 auto auto 0 round 12px)` animating
`offset-distance` from 0% to 100% over 3s travelled 281.7px along the top edge in 900ms. WebKit
resolves the declaration and serializes it back as `inset(0px 0% 0% 0px round 12px)`, which is the
same path. The conic-gradient fallback below is therefore not built; it stays recorded for the
platforms this series does not test.

**The fallback holds only for a near-square shape.** Measured later, while the corner stall in D5 was
still unexplained: on a rectangle as wide as this canvas a centre-anchored sweep varies its speed around
the border by roughly fifty times, because equal angles map to unequal arc lengths. It is a fallback for
a card, not for this canvas.

**Fallback if it does not resolve:** a rotating conic-gradient mask. A square element larger than the
rectangle, filled with `conic-gradient(from 0deg, transparent 0turn, <color> 0.08turn, transparent
0.16turn)`, animated with `transform: rotate(1turn)` and clipped to the border ring by the same mask the
container already applies. It produces a sweep around the border with no `offset-path` at all, and
`transform` and `conic-gradient` have no support question anywhere this ships. It differs visibly - the
sweep's speed is angular, so it moves faster along the short edges - which is why it is the fallback and
not the first choice.

### D4 - `over` drives the mark, not `enter`

`enter` fires once, when the drag crosses into the **window**. A drag that arrives over the navbar and
then moves down onto the canvas emits no second `enter`, so an implementation keyed on `enter` alone
would never light up for it - and one that lit up on `enter` regardless of position would light up for
drags that never reach the canvas.

The rule is therefore: recompute from the position on both `enter` and `over`, and clear on `leave` and
`drop`. The mark then tracks the canvas rectangle continuously, appearing when the pointer crosses into
it and disappearing when it crosses out, without either edge being a special case.

`leave` carries no position, which is why it clears unconditionally rather than being hit-tested.

### D5 - The canvas's own border, drawn by the beams alone

The drawn region is a child of the canvas: `absolute inset-0 border border-border`, with four
`BorderBeam`s inside it. Two things follow from the design rather than from taste:

- **No visible track, after one was tried.** A `--border` line was drawn under the beams first, on the
  structure the design gives its analysing spinner - `border: 2px solid #27272a` with coloured arcs
  over it, and `#27272a` is `--border`. What works on a 32px spinner does not survive being stretched
  around a canvas: at that size the line reads as a grey box being drawn around the window rather than
  as the ground under a light. It is gone, and the shape is carried by the beams being long instead -
  four of them at 384px, about a third of the border lit at any moment, so no edge is ever the only
  one with something on it.
- **`rounded-xl` corners, and the beams' path left alone.** Upstream rounds the path a
  beam travels by the beam's own **length**, and that is load-bearing rather than sloppy: a beam is a
  rigid square that cannot bend, so the only corner it can follow is one whose curve is scaled to it.
  Holding the path to the region's 12px instead - to make the beam hug the border exactly - is what
  makes a beam stall at every corner. A `radius` prop exists for a caller who wants the tighter trace
  on a small box, and it defaults to `size`.

It is `pointer-events-none` and `aria-hidden`, which is what makes the spec's "carries no text, not
announced, not reachable" structural. Critically, it must not intercept the drag: a full-bleed element
over the canvas that captured pointer events would break the drop it exists to advertise.

**Reversed after seeing it in the window.** This decision first read: an inset rounded rectangle,
`inset-4 rounded-xl`, on the reasoning that the canvas's own left, right and top edges are the window's
and a beam along them would read as window chrome. Drawn, it read as something worse - a second
rectangle floating inside the canvas, marking out a region that accepts nothing, while the region that
does accept the drop went unmarked. The rule the spec states is that what is offered and what is taken
up are the same rectangle; an inset mark is that rule broken by 16 pixels. The beams trace the region,
so they trace its edges, and the window-chrome worry is answered by their only being drawn while a drag
is actually in flight.

**The corner stall, and where it actually came from.** The first version of this held the beams' path
to the track's 12px radius, and every beam visibly stopped at each corner. Measured in the running
window by tracking the lit head frame to frame from a 30fps capture, at 192px over a 4s lap:

| path radius             | advance through a corner   |
| ----------------------- | -------------------------- |
| the region's 12px       | 49% of straight-edge speed |
| upstream's, = `size`    | 130%                       |

The fix was to stop overriding upstream. MagicUI's own component was installed beside this one for the
comparison, `motion` and all, and measured identically to the port at the same radius - 64px per frame
against 63, both reaching 130% through a corner. That settles two things: the corner behaviour belongs
to the path rather than to whatever drives `offset-distance` along it, so D2's CSS keyframe is not
implicated; and a divergence from upstream taken for a good-looking reason is worth measuring before it
is kept. `motion` was removed again once the comparison was done.

**The conic sweep is not the escape hatch D3 imagined.** Built and measured beside the beams while the
stall was still unexplained: anchored at the centre of a rectangle as wide as this canvas, the sweep's
speed around the border varies by roughly fifty times - a crawl at the middle of each long edge, a whip
through each corner - because equal angles map to wildly unequal arc lengths. It holds together on a
near-square card, which is where the technique is usually seen. D3's fallback stands only for a shape
close to square, and this is recorded there.

`size` is 384px and `duration` 4s, both settled by eye against the real window; `borderWidth` is 2px,
the width the design's analysing spinner draws its own ring at. With the path radius left as upstream
sets it, length no longer trades against the corners: at 384px the head still passes a corner at 144%
of its straight-edge speed.

## Risks / Trade-offs

**[`offset-path: rect()` may not resolve in WKWebView]** → D3. Verified first, with a
`conic-gradient` fallback that has no support question. This is the one risk that could change the
component's internals, which is why the check is task 1 rather than a review comment.

**[The mark appears for drags carrying nothing openable]** → Accepted and specified. It says where files
are taken, not which files are acceptable, and the second answer already arrives by name after the drop.
The alternative needs an async `inputExtensions()` resolved mid-drag, which appears late on a slow drag
and not at all on a fast one - a signal that is sometimes right is worse than one that is consistently
about something else.

**[An overlay over the canvas could swallow the drag or the zoom gestures]** → `pointer-events-none` on
the drawn region and on both beams. Worth an explicit test rather than trusting the class, because the
failure - a drop that silently stops working - is exactly what this change was meant to make legible.

**[No `prefers-reduced-motion` handling]** → Known gap. The beam is transient, lasting only while a drag
is in flight, which is the weakest case for suppression; the particle field in `add-gui-canvas-background`
is the stronger one and carries the same gap. If either is addressed, both should be.

**[The animation runs while a drag is in progress, which is when the pointer is busy]** → No real cost.
Two composited elements animating one property each, for a second or two.

## Open Questions

None. The implementation route, the geometry, the trigger, the colours and the token name were all
settled before this was written.
