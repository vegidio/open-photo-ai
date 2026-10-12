## Context

See proposal.md - Why. What matters for the approach:

- **The surface is a string today.** `lib/constants.ts` exports `DOTTED_CANVAS`, three Tailwind classes
  applied to one element in `features/preview/Preview.tsx`. Its own comment says it is a constant rather
  than a component *"because it is applied to an element that already exists"* and anticipates the crop
  dialog as a second reader.
- **The settings dialog draws four row shapes.** `features/settings/rows.tsx` has `SelectRow`,
  `SwitchRow`, `ButtonRow` and `SliderRow`, all built on one private `SettingsRow` frame: a label, a
  control in a `h-[34px] w-[196px]` column, a muted description underneath. The design's Background row
  is a fifth shape, and it keeps the same 196px column - two radio options in a `1fr 1fr` grid inside it.
- **`sections.ts` is the single table both dialog panes read.** A setting has to appear in it or the
  navigation list and the section pane disagree; `SettingsDialog.test.tsx` walks it.
- **`stores/settings.ts` enforces its own completeness.** `SNAPSHOT_KEYS` is `as const satisfies readonly
  (keyof SettingsData)[]`, and the `Uncovered` type alias turns a field added to `SettingsData` but left
  out of the snapshot into a compile error naming the field. Adding `background` without listing it will
  not build, which is the intended outcome.
- **The design ships the implementation it wants.** `particles.js` in the design project is a hand-port
  of MagicUI's component as a web component, and it differs from upstream in ways that are edits rather
  than accidents - see D3.

## Goals / Non-Goals

**Goals:**

- One place decides what the canvas surface is, read by the preview now and by the crop dialog when it
  is built, so the second reader inherits the choice rather than a copied string.
- The particle field costs nothing that is not being looked at.
- The new row is a row like the other four - same frame, same column width, same anchor-id contract -
  rather than a bespoke block inside the Application section.

**Non-Goals:**

- The Crop/Rotate dialog. This change gives it a surface to read; it does not draw it.
- A background chosen per image, per window or per comparison mode. One application-wide preference.
- Any third surface, or user-supplied colours, counts or speeds. Two named options, the design's
  parameters, no tuning.

## Slices

This is a single change, not a slice of a series. It has no predecessor and blocks nothing; the crop
dialog is a separate future change that will read what this one establishes, not a continuation of it.

## Decisions

### D1 - `DOTTED_CANVAS` becomes a hook, not a conditional string

The constant becomes `useCanvasSurface()` in `lib/` (or a small `CanvasBackground` component wrapping
both halves), returning the class string for the dotted case and rendering the particle canvas for the
other. A hook rather than an exported `background: Background` plus two branches at each call site,
because there will be two call sites and the branch is exactly what must not be duplicated between them:
the preview and the crop dialog have to agree, and the way they agree is by asking one question rather
than by two components each remembering to ask both halves of it.

It stays in `lib/` rather than moving into `features/preview/`, for the same reason the constant did -
its second reader is in another feature, and a `use` reaching from `features/crop/` into
`features/preview/` is the front-end form of the cross-model import the project layout rules forbid.

**Which** value it reads is D8: what is in force, not what is preferred.

**Alternative rejected:** leaving `DOTTED_CANVAS` and adding a sibling `PARTICLE_CANVAS`, with the
branch at each call site. Two call sites, two chances to branch differently, and the design's two
regions are meant to be identical.

### D2 - The particle canvas is a sibling of the canvas content, not a background layer of it

The design's markup settles this: the particle element is a child of the canvas div, placed before the
empty state and the loaded panes, and every child that follows gains `position:relative; z-index:1`.
That is followed literally. The particle canvas is `absolute inset-0 pointer-events-none`, the content
above it, and the canvas div keeps `bg-background` in both cases - only `background-image` differs.

`pointer-events-none` is what makes the spec's "does not respond to the pointer" true structurally
rather than by having registered no handlers: the wheel-zoom and drag-to-pan handlers slice 4 put on the
canvas must keep receiving events with the field drawn, and a full-bleed canvas element that could
swallow them is the failure this prevents.

### D3 - The upstream component is adapted, not pasted, in three ways

MagicUI's `particles` registry entry declares no npm dependencies, so it is vendored into
`components/ui/particles.tsx` like every other shadcn component here. Three changes, each with the
design's own port as precedent or a local reason:

1. **Mouse magnetism removed entirely.** Upstream keeps a `MousePosition()` hook - `useState` driven by
   a `window` `mousemove` listener - and leans each particle toward the cursor. The design's port keeps
   the arithmetic with the term multiplied out, `c.magnetism * 0`, and registers no listener. Dropping
   the hook rather than zeroing the term removes the re-render, which is the actual cost: a `useState`
   write per pointer move re-renders this component on every frame of a pan gesture over the canvas.
   The `staticity` prop goes with it, since nothing reads it once the term is gone.
2. **`ResizeObserver` in place of the debounced `window` resize listener.** Upstream re-initialises
   200ms after the *window* resizes. This canvas resizes when the window does not - the sidebar is
   fixed-width and the drawer overlays rather than displaces, but the window's own resize is not the
   only thing that moves the canvas's box, and a future panel would make that worse silently. The
   frontend already has `hooks/useResizeObserver.ts`.
3. **The animation loop is gated on window focus.** See D4.

Everything else is upstream's, including the per-particle `targetAlpha`, the edge fade and the respawn
when a particle leaves the box. The design's `quantity=120`, `color=#ffffff` and `size=0.6` are carried
over rather than upstream's `100` / `0.4`.

**Alternative rejected:** consuming the design's `particles.js` as a custom element. It is a web
component written for the design canvas, it duplicates what the React version already does, and a
`customElements.define` inside a React tree is a second component model in a codebase that has one.

### D4 - Focus, not visibility, gates the loop

`requestAnimationFrame` is already throttled to a stop by the browser when a tab is hidden, so
`document.visibilityState` buys nothing here - and in a desktop application the window is *visible* far
more often than it is *focused*. The gate is Tauri's window focus, via
`getCurrentWindow().onFocusChanged`, falling back to the DOM's `focus`/`blur` on `window` for the test
environment.

Stopping cancels the pending frame and leaves the last one painted, which is what the spec's "still
drawn while unfocused" means; resuming schedules a new frame from the same particle array, so nothing
restarts. Particle positions are in a ref, not state, so there is nothing to preserve across the gap.

**The initialiser paints one frame itself**, rather than leaving the first to the loop. The gate stops
by leaving the last frame painted, precisely so the field is still drawn while the window is away - and
a field that has never painted a first frame has no last one to leave. Without it an application
launched and switched away from before its window came up would show a bare canvas until the user came
back to it, which is the same blank surface the gate is careful to avoid, arrived at from the other
side.

**Alternative rejected:** tearing down and re-initialising on focus. It would repopulate the array and
the field would visibly reshuffle every time the user came back to the window.

### D5 - The radio row keeps the 196px control column

`RadioRow` is added to `rows.tsx` as the fifth shape on the same private `SettingsRow` frame. The design
draws the two options in a `grid-template-columns:1fr 1fr` inside a 196px box - the same width every
other control in the dialog gets - so the row stays aligned with the eleven selects above and below it
rather than being a wider block that breaks the column.

shadcn's `radio-group` is pulled from the registry, which `radix-ui` already satisfies as a dependency.
Per the project's standing note, the generated file needs its `cn` import repointed, its React import
made a named import, its components turned into arrow functions, and the bogus `cn` package the
generator adds removed from `package.json`.

**Alternative rejected:** `ToggleGroup`, which is already vendored and would need no new component. The
design draws circles with filled dots - radio semantics, and a screen reader should hear "radio group,
Particles selected" rather than a pressed toggle. The [tooltip-aschild-clobbers-data-state] lesson also
applies to toggle items in a way it does not to radios.

### D6 - The preference applies on Save, like every other one

`gui-settings` already requires that nothing outside the surface observes a draft, and names the
language as the worked example. The background is the same shape of thing: the canvas is outside the
dialog, so it keeps its surface until `commitDraft` runs.

This is worth stating because the design's mock toggles the canvas live as the radio is clicked. That is
a mock demonstrating two states on one page, not a claim about draft semantics - the same mock also
shows every dialog open at once. Applying live would make Cancel unable to restore what the user saw,
which is exactly the failure the existing requirement was written against.

### D7 - The default is Particles, and the repair target is Particles

`background: "particles"` as the store's initial value, listed in `SNAPSHOT_KEYS`, and repaired in
`merge` the way `processor` and `language` already are: a persisted value naming neither surface reads
as particles rather than being kept.

The store persists under the Wails application's `settings-storage` key, which it shares. That
application has no background setting, so an upgrading user's stored object simply has no `background`
field, and `merge` gives them the default - which is the intended outcome rather than a migration.

### D8 - What is *in force* is a second store, not a read of the preference

D6 says the background applies on Save. This is how, and it is the one structural thing in this
change that the first draft of this document did not anticipate.

Every settings row writes `stores/settings.ts` the moment a control is touched; what the draft gate
closes is *storage*, not the store. That is invisible for the eleven preferences nothing outside the
dialog reads, and for the language, which reaches the interface only when `useSettingsDialog` hands
it to i18next on Save. The background is the first preference whose reader is **a component
subscribed to a store** - so a canvas reading `settings.background` would change surface as the radio
was clicked, and Cancel could only change it back afterwards, having already shown the user the other
surface. That is exactly the failure D6 exists to prevent.

So `stores/canvas.ts` holds the background as *applied*, `useCanvasSurface` reads it rather than the
settings store, and `useSettingsDialog.close(true)` publishes into it on the line beside
`i18n.changeLanguage`. i18next is the same shape of thing for the language: the settings store holds
the draft, something outside it holds what is in force.

It is deliberately **not persisted**. The preference is written once, by the settings store, under
the key it shares with the Wails application; a second copy on disk would be a second thing to repair
on rehydrate and a second thing to disagree with the first. It is seeded at module load from the
committed preference - `persist` reads storage synchronously as `stores/settings.ts` is imported, and
that import is above it - and it subscribes to `onFinishHydration` so a rehydration moves it too.

**Alternative rejected:** reading `settings.background` in `useCanvasSurface` and having the dialog
hold the pre-draft value to restore on Cancel. That is the same recording `beginDraft` already makes,
kept a second time in a second place, and it puts the surface the user sees at the mercy of a Cancel
path running - which, unlike a draft in the store, cannot be undone by the process simply ending.

## Risks / Trade-offs

**[A `requestAnimationFrame` loop runs behind ONNX inference]** → Accepted, with the focus gate as the
only mitigation. 120 particles is a few hundred arithmetic operations and one `arc` fill each per frame;
it is not free, but it is far below what the canvas already does drawing a 20-megapixel photograph. The
two heavier mitigations - pausing while a photograph is open, and honouring `prefers-reduced-motion` -
were both considered and declined, because each is a third state the design does not draw. If inference
throughput measurably moves, pausing while an image is open is the first thing to add and costs almost
nothing, since the field is mostly occluded in that state anyway.

**[No `prefers-reduced-motion` fallback]** → Known gap, recorded rather than defended. The argument
against it is that it would override a choice the user just made by hand; the argument for it is that a
drifting field behind a photograph is exactly what that setting exists to suppress. Cheap to add later -
one media query in the hook - and it changes no requirement, only which surface is drawn.

**[The particle field competes with the photograph being judged]** → The whole application exists to let
someone see what a model did to an image, and animated dots behind it are a distraction by construction.
Mitigated by the setting itself: anyone this bothers turns it off once. It is also why the default
question was asked rather than assumed, and why there is no third, busier option.

**[`ResizeObserver` loops]** → Re-initialising the canvas inside a resize callback that itself changes
nothing about layout cannot feed back, because the canvas element is absolutely positioned and sized in
percentages. Worth keeping true: the observer must not write anything that affects its own box.

## Open Questions

None. The default, the magnetism, the focus gate and the applies-on-save rule were all settled before
this was written.
