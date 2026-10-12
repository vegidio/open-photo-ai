## Why

The preview canvas has exactly one surface and always has: a `--surface-dot` dot every 48px over
`--background`, written as a constant string in `lib/constants.ts` and applied to one element. Nothing
chooses it, because until now there was nothing to choose between.

The design project now draws a second one. Its Settings dialog gains a **Background** row under
Application, between Language and Logs, offering *Particles* and *Dotted*; choosing Particles replaces
the dotted grid with a drifting field of them, and the design ships `particles.js` - a port of
[magicui.design/docs/components/particles](https://magicui.design/docs/components/particles) as a web
component - to draw it. The same pair is drawn on the Crop/Rotate dialog's canvas, which is a later
slice's surface, so the choice is one two regions will read rather than one the preview owns.

## What Changes

- **The Application section gains a Background setting**, a two-option radio between **Particles** and
  **Dotted**, placed between Language and Logs exactly as the design orders it. It is the fifth row
  shape in the dialog - the four today are select, switch, button and slider.
- **Particles is what a fresh install gets.** The dotted surface remains, chosen rather than assumed.
- **Choosing Particles replaces the dots rather than layering over them.** The design sets
  `background-image: none` and draws the particle canvas; the background colour is unchanged either
  way, so the two options differ in exactly what is drawn over `--background` and in nothing else.
- **The particle field is inert.** It drifts on its own and ignores the pointer. See Impact - this is a
  deliberate departure from the upstream component, and the design's own port made it first.
- **The animation stops while the window is not focused** and resumes when it is. Nothing else gates
  it: it runs behind an open photograph, as the design draws it, and it does not consult
  `prefers-reduced-motion`.
- **The canvas surface becomes a value two regions read.** `DOTTED_CANVAS` stops being a constant. Its
  own comment already anticipated the second reader - *"the crop dialog reuses the same surface in a
  later slice"* - and this change is what makes that reuse a preference rather than a copied string.
- **The choice is a draft until Save, and survives a restart**, by the mechanism every other preference
  in the dialog already uses. Cancel puts it back.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-settings`: gains the Background preference - that the application offers a choice of what is
  drawn behind the image, that it is one of two named surfaces, that it defaults to the particle field,
  that a remembered value naming neither is repaired, and that it is a draft until saved like every
  other setting the dialog holds.
- `gui-shell`: gains what the preview canvas actually draws. The surface was never specified - the
  existing requirements say the canvas invites an image and then draws one, and say nothing about what
  is behind either - so this adds the promise that the canvas carries the chosen background, that the
  choice takes effect without a restart, and that the invitation and the photograph are both drawn over
  it rather than behind it.

## Impact

**Crates: `gui` only, and the React frontend alone within it.** No Tauri command, no IPC, no Rust. The
preference is stored the way the other five are - `zustand/persist` into `localStorage` under the Wails
application's `settings-storage` key - so there is no new wire format and nothing for `opai` to hold.

Nothing here is shared logic trapped in a front end. Which of two decorative surfaces a webview paints
behind a photograph is a fact about a window; the terminal front end has no canvas to paint on, and
`opai` knows nothing about either. Compare the analytics row, which persists a decision nothing reads
yet *because* a later consumer will - there is no equivalent second consumer here, now or later.

**`rust-sak`: nothing is needed and nothing is missing.** This change decodes nothing, downloads
nothing, hashes nothing and touches no file.

**New npm packages: none.** MagicUI's `particles` registry entry declares no dependencies - it is React
hooks and a 2D canvas - so it is vendored into `components/ui/` the way every other shadcn component in
this frontend already is, and adapted rather than pasted (see the divergences below). `radio-group` is
pulled from the shadcn registry, whose Radix dependency `radix-ui` already satisfies.

**i18n: four new keys** - the row's title and description, and the two option labels - added to all
thirteen locales, not only `en.json`.

**Deliberate divergences, each argued in design.md.** These are from the *upstream component*, not from
the Go application, which has no equivalent - the Wails app draws one surface and offers no choice:

- **No mouse magnetism.** MagicUI's `Particles` registers a `window` `mousemove` listener that drives
  React state on every pointer move, and leans each particle toward the cursor weighted by `staticity`
  and a per-particle `magnetism`. The design's own port keeps the arithmetic and multiplies it out -
  `c.magnetism * 0` - and registers no listener at all. That zero is an edit, not an oversight, and it
  is followed here: a `mousemove`-driven re-render spanning the whole window is the most expensive
  thing this feature could do, for an effect that competes with the photograph being judged.
- **A `ResizeObserver` in place of a debounced `window` resize listener.** Upstream re-initialises the
  canvas 200ms after the *window* resizes. This canvas changes size when the window does not: the
  sidebar and the drawer are laid out over it. The frontend already has `useResizeObserver` for exactly
  this.
- **Paused while unfocused.** Upstream runs its `requestAnimationFrame` loop unconditionally. A
  backgrounded photo editor holding a core awake to animate something nobody is looking at is the one
  cost here with no defence, and `document.visibilityState` is not enough - an unfocused but visible
  window is the normal case on a desktop.

**Consciously not done**, so a later reader knows it was considered rather than missed:

- **No `prefers-reduced-motion` fallback.** It would be a third state the design does not draw, and the
  setting it would override is the one the user just chose by hand. Recorded here as a known gap rather
  than argued as correct.
- **No pause while a photograph is open.** The design draws the field running behind the loaded panes,
  where the image plates occlude most of it, and that is followed rather than optimised.
- **The Crop/Rotate dialog is not built here.** It is a later change. What this one owes it is a surface
  it can read rather than a string it would have to copy, and that is delivered.

**One unrelated thing this change does not carry.** The same design revision moved the navbar's
Dimensions readout from 11px to 13px over 12px, added `gap:2px` and `margin-left:8px`. It belongs to
neither this change nor the drop-beam one and is left for a separate pass, recorded here so the design
diff is fully accounted for.

**Task count: 14**, against the 10 first estimated. The store's three concerns - the field, its repair
and its draft behaviour - are three tasks rather than one, and the shadcn `radio-group` install is its
own step ahead of the row that uses it. Still inside the limit; this is a single change, not a slice of
a series.
