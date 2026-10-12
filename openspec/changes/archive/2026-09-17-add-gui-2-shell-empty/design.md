## Context

See proposal.md - Why. The starting point is what slice 1 left: `frontend/` holds `App.tsx` (the
placeholder), `main.tsx`, `style.css`, `ipc/app.ts`, `i18n/` with thirteen catalogues, `lib/utils.ts`
and `components/ui/` with the seven generated shadcn primitives. `tsconfig.json` and `vite.config.ts`
both map `@/*` to `./frontend/*`. There is no `features/`, no `stores/` and no component that draws
anything.

Three things outside this crate are the authority for what gets built:

**Screen 01 in the design project** (`Open Photo AI (screens).dc.html`, entry
`{ num: '01', title: 'Empty state' }`) states the appearance: a 48px navbar on `--card` with a
`--border` bottom rule; the canvas on `--background` with a 1px `--surface-dot` radial dot every
48px; a 176px drawer translated down by 128px so only its 48px header shows; a 256px sidebar on
`--card` with a `--border` left rule, a 144px preview area, a 24px-padded Autopilot block on
`--background`, and a 48px Export bar on `--secondary` at 35% opacity.

**The Wails sources** are the authority for behaviour, and every metric above is already theirs:
`features/navbar/Navbar` is `min-h-12` with `pl-[86px]` on darwin; `features/drawer/Drawer` is
`drawerBleeding = 48` and `drawerHeight = 128` with `translateY(128px)` when closed;
`Preview` is `h-[calc(100%-48px)]` over `DOTTED_BACKGROUND`; `Sidebar` is `w-64`. The disabled
conditions are theirs too - `DrawerToggle`, `DrawerSelectAll`, `PreviewSelector` and `DrawerZoom` all
take `disabled={fileCount === 0}`, `AddEnhancement` takes `disabled={fileLength === 0}` and
`SidebarExport` is disabled while nothing is selected.

**`cmd/gui/main.go`** settles the platform question: `MacTitleBarHidden` with
`InvisibleTitleBarHeight: 50`, and no `Frameless` on any platform. macOS hides its title bar and
lets the top strip drag the window; Windows and Linux keep their decorations.

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | done |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs. Placeholder `App.tsx` deleted | 1 | **CURRENT** |

**Slice 2 is this change.** The remaining 21 designed screens are not yet cut into slices; they are
cut when they are reached, and the table carries only what has been committed to.

**Crates.** `gui` only: `tauri.conf.json`, `src/lib.rs`, `capabilities/default.json` and everything
under `frontend/`. `opai` and `cli` are untouched. Nothing in this slice is logic either front end
could share - see proposal.md - Impact.

**rust-sak: no feature is used, and none applies.** The only Rust this change adds is a plugin
registration. Nothing in it is a candidate for extending rust-sak.

**New crates.** `tauri-plugin-os` (2.x), for the reason in the platform decision below. Nothing else.

## Goals / Non-Goals

**Goals:**

- The frame every later screen is built inside, at the metrics both the design and the Wails app
  agree on, so a later slice adds its feature and not a layout.
- A platform answer that is available before first paint, so the macOS inset is never applied a frame
  late.
- Component boundaries that match the screens to come: a later slice changes what a region contains
  without touching the other three.

**Non-Goals:**

- **Opening files, by any route.** No file picker behind Browse images or Add images, and no
  drag-and-drop handling - Tauri's `onDragDropEvent` is not wired. The canvas states the invitation
  because that is what screen 01 shows; honouring a drop needs a file list, image decoding and a
  preview, which is the next slice's whole subject. Wiring the event now would mean deciding the file
  store's shape against a guess.
- **A file store.** Same reason. See the decision on where the empty state's truth lives.
- **The Settings and About dialogs, and the navbar's current-file and Dimensions blocks.** Screens
  16, 17, 19 and the `hasFile` branches of screen 01's own template. The buttons render because the
  navbar's layout is fixed by where they sit; they do nothing yet.
- **The update check.** `IsOutdated()` is a network call with no counterpart in `opai` yet, so the
  "Update Available" button screen 01 draws is not rendered at all rather than rendered permanently
  dark.
- **Language switching and any settings persistence beyond Autopilot.** Slice 1 fixed i18next to `en`
  and said the settings slice adds the rest; that has not changed.

## Decisions

### The macOS title bar is configuration, and the inset is the only code

`tauri.conf.json`'s main window gains `"titleBarStyle": "Overlay"` and `"hiddenTitle": true`. Both
keys are macOS-only in Tauri's `WindowConfig` and are ignored elsewhere, so one unconditional
configuration produces the macOS-only behaviour and needs no per-platform build.

`Overlay` rather than `Transparent`: `Transparent` keeps the title bar and makes it see-through,
which leaves the traffic lights floating over whatever is behind them and the window's height
unchanged. `Overlay` removes the bar and hands its area to the webview, which is what
`MacTitleBarHidden` does in Wails and what puts the traffic lights inside the navbar.
`hiddenTitle: true` suppresses the title text that would otherwise be drawn across the navbar.

The navbar then carries `data-tauri-drag-region`, because a window whose title bar is gone has
nothing else to drag it by. Tauri's own handler also gives it double-click-to-zoom, which is what the
removed title bar did. The attribute goes on every platform: on Windows and Linux it costs nothing
and makes the navbar behave like the header bar it looks like.

*The attribute alone is not enough.* `core:window:default` grants 28 permissions and
`allow-start-dragging` is NOT among them, so `capabilities/default.json` gains it explicitly. Without
it the attribute is inert and the drag fails silently - no console error, no rejected promise,
nothing to find - which on macOS leaves a window with no title bar AND no way to move it. That it is
absent from the default set is easy to assume the other way round, because the neighbouring
`allow-internal-toggle-maximize` IS granted: double-click-to-zoom works on an undraggable window,
which makes the strip look wired up when it is not. Verified by dragging the running window rather
than by reading the manifest.

*The inset.* On macOS the navbar gets 86px of left padding, the value the Wails app uses, so the
application name starts clear of the traffic lights. Nowhere else.

*The traffic lights are moved down to meet it.* `Overlay` hands the title bar's area to the webview
but leaves the buttons where macOS puts them - vertically centred for its own ~28px title bar, which
inside a 48px navbar reads as hugging the top. Screen 01 centres them, so the window gains a third
macOS-only key, `"trafficLightPosition": { "x": 9, "y": 26 }`. `x` is the close button's left edge and
9 is the value macOS already used, so nothing moves horizontally.

`y` is NOT the button's offset from the top, and the name invites reading it that way. Tauri forwards
it to wry's `with_traffic_light_inset`, which resizes the title-bar container to
`button_height + y` and leaves the buttons centred inside it; the 12px buttons' centre therefore lands
at `y - 2.5`, so `y = 26` centres them on 23.5 - the middle of a 48px bar. That relationship was
derived by measuring the running window at two values rather than from the documentation, and the
first attempt (`y = 18`, reasoning about it as a top offset) happened to reproduce the macOS default
exactly and so looked like the key doing nothing at all.

**This value is bound to the navbar's 48px height.** Changing one without the other silently
un-centres the buttons, and no test will say so.

*This is where the app stops matching the Wails one, and it is appearance rather than behaviour.*
Wails sets `InvisibleTitleBarHeight: 50`, which is a drag area and nothing else - it does not move the
buttons, so they sit high there too. The design is the authority for appearance and it centres them,
which is the rule proposal.md - Impact already states rather than an exception to it.

### The platform comes from `@tauri-apps/plugin-os`, read synchronously

`frontend/ipc/os.ts` wraps the plugin's `platform()` and exports the one question the frontend
actually asks - whether this is macOS.

The reason is first paint. The existing `version` command is a `#[tauri::command]`, so `appVersion()`
is a promise and the version arrives a tick after the first render - which is fine for a string in
the corner and not fine for 86px of padding, because the navbar would render flush, then jump. The os
plugin does not use a command for this: `tauri-plugin-os`'s `init()` builds an
`InitJavascript` from `std::env::consts` and installs it with `js_init_script`, so
`window.__TAURI_OS_PLUGIN_INTERNALS__.platform` is populated before any of our code runs and
`platform()` is a synchronous property read. No promise, no layout shift.

That also settles the plugin's capability: `platform` is not a command. The plugin's only commands
are `locale` and `hostname`, which nothing here calls, so the plugin's `os:default` permission set is
deliberately not granted - granting it would allow six reads the application does not make. This
plugin therefore adds nothing to `capabilities/default.json`; the one permission that file does gain
is the window drag above, which is unrelated to it.

*Alternatives considered.* **A `#[tauri::command] fn platform()` of our own**, matching `version` -
the same file, the same test shape, one fewer dependency. Rejected for the flash above, and because
making it synchronous would mean writing our own `js_init_script`, which is reimplementing the plugin
in this crate. **Vite's `define` over `TAURI_ENV_PLATFORM`**, which Tauri sets for both
`beforeDevCommand` and `beforeBuildCommand` - a compile-time constant, zero runtime cost, and no
dependency at all. Rejected because it silently produces the wrong bundle whenever Vite is run
outside Tauri (`pnpm dev` in a browser, `pnpm build` in CI), and the failure is a layout that is
subtly wrong rather than an error. **`navigator.userAgentData`** - not a fact about the host the app
runs on, and deprecated surface besides.

*What this costs in tests.* jsdom has no `__TAURI_OS_PLUGIN_INTERNALS__`, so a bare `platform()`
throws there. That is exactly why the call is wrapped: `ipc/os.ts` is the single module that touches
the global, component tests mock `@/ipc/os` the way `App.test.tsx` already mocks `@/ipc/app`, and
`ipc/os.test.ts` stubs the global directly to pin the wrapper itself.

### The empty state's truth lives in the shell, as props - not in a file store

Every disabled control in screen 01 is disabled for the same reason: there are no files. In the Wails
app each component reaches into `useFileStore` for `files.length` itself. Here the shell passes
`disabled` down instead.

The point is that a file store written in this slice would be written against a guess. Its shape is
decided by the slice that loads files, hashes them, tracks the current one and the selection - and
that slice would then have to rewrite whatever this one invented. Slice 1 made the same call about
Zustand generally, and the reasoning has not changed just because a screen now exists.

A `disabled` prop is also the honest contract for these components: `DrawerToggle` does not care *why*
it is disabled. When the file slice arrives it changes one place - the shell - from a literal to a
store read, and no leaf component changes at all.

*Alternative considered.* A minimal `useFileStore` holding `files: []` with no way to add any. It
reads as the real thing and is not, and every component written against it would be written against a
shape nothing has validated.

### One store, and it is the one the shell actually reads

`frontend/stores/enhancements.ts` holds `autopilot`, `setAutopilot` and `toggle`, defaults to `true`,
and persists under `enhancements-storage`.

This is not a guess about a future store's shape: it is the Wails store's persisted surface exactly.
That store holds four fields and three of them are Maps of per-file state, but its `partialize` keeps
only `autopilot` - so the file that lands here now is byte-for-byte the part that survives a restart,
and the later slice adds the Maps beside it without touching what this one wrote or what it saved.

Autopilot is the only live control on screen 01, and it is deliberately not gated on having a file:
it is a standing preference about what should happen when one is loaded.

### Components are grouped by feature, one file each

`frontend/features/navbar/Navbar.tsx`, `features/preview/Preview.tsx`,
`features/preview/PreviewEmpty.tsx`, `features/drawer/Drawer.tsx`, `features/drawer/DrawerHeader.tsx`
and `features/sidebar/Sidebar.tsx`, with each test beside its component.

The feature grouping is the Wails app's and it earned it - `features/<area>/` is how twenty-three
screens stay navigable. What is not carried over is the directory-per-component form
(`features/navbar/Navbar/index.tsx`): it doubles the path depth, makes every editor tab read
`index.tsx`, and exists in the original for no reason this project inherits. Tests beside the code
they cover is already the convention on the Rust side of this repository.

The drawer's header controls stay inside `DrawerHeader` in this slice rather than becoming six files.
Each is a few lines and all six are disabled; they split when one of them gains behaviour, which is
the slice that gives them something to act on.

### The canvas is inset; the drawer is drawn over it

The canvas is absolutely positioned with `bottom-12` (48px) and the drawer with `bottom-0` and
`translate-y-32` (128px) when folded, over a 176px height. Both are the Wails values and both are what
screen 01 draws.

The drawer's height is the one metric here that is not a Tailwind class: it is `DRAWER_BLEEDING = 48`
plus `DRAWER_HEIGHT = 128` in an inline style, because those are the two numbers the Wails app names
(`drawerBleeding`, `drawerHeight`) and the fold distance has to stay equal to the second of them. A
literal `h-44` states the sum and hides the relationship, so the fold and the height could drift
apart silently; the constants make the header's 48px and the body's 128px the things that are
written down, and the total the thing that is derived.

The consequence is worth stating because it is not obvious: the canvas is *permanently* 48px short of
the bottom, whether the drawer is folded or not. Unfolding slides the drawer up over the canvas and
the canvas does not move. That is what makes folding cheap - nothing below it re-lays-out, and the
preview image never rescales - and it is why the spec makes it a requirement rather than an
implementation detail.

### Three icons are ours; the rest are lucide

The design draws almost everything with lucide: `FolderOpen` for the empty canvas, `Plus` for Add
images and Add enhancement, `Plus`/`Minus` for zoom, `ChevronsUp` and `ChevronsDown` for the drawer's
folded and unfolded states, `Upload` for Export. Those are used directly, at the stroke 1.5 the
provider set in slice 1 - except the empty-canvas folder, which the design draws at stroke 1.25 and
72px, so it overrides both on itself.

The three preview-mode icons are not in lucide and are not in any icon set: they are diagrams of the
three comparison modes, and the Wails app drew them as custom SVG too. They go in
`frontend/components/icons/`, as the design's paths, authored at lucide's geometry - 24x24 viewBox,
`currentColor`, `stroke-width` from the same provider - so they sit in the toggle group beside real
lucide icons without looking imported from somewhere else.

### One token is added

The design paints the Autopilot label `#4ade80` and its switch track `#22c55e`. `--success` in
`style.css` is `#22c55e`, so the track is covered and the label is not. `--success-bright: #4ade80`
joins the existing block of values that are the design's but outside shadcn's vocabulary
(`--surface-dot`, `--foreground-faint`, `--success`, `--warning`), and is re-exported through
`@theme inline` with them.

The alternative - reaching for Tailwind's `text-green-400` at the call site - would put a palette
literal in a component in a codebase where every other colour is a token, and would be the first one.

### The placeholder goes, and its IPC stays

`App.tsx` and `App.test.tsx` are replaced: the file becomes the shell's composition root and its test
asserts the four regions are present. `frontend/ipc/app.ts` and `ipc/app.test.ts` are untouched and
the `version` command keeps its job - the navbar renders `navbar.version` from it, so the path the
placeholder existed to prove is still exercised, now by something that is not a placeholder.

### No new i18n keys, and `preview.empty.title` needs `Trans`

Every string is already in the thirteen catalogues. One of them,
`"Drag and drop images<br/>to start editing them"`, carries markup: the line break is part of the
centred two-line layout and lives in the catalogue so a translator can move or drop it. `<br/>` is one
of i18next's default `transKeepBasicHtmlNodesFor` nodes, so it is rendered with `<Trans>` rather than
`t()`, which is what the Wails app does for the same key and the same reason.

## Risks / Trade-offs

- **`titleBarStyle: "Overlay"` is verifiable only on macOS.** → It is one configuration key with an
  existing Wails counterpart, and the verification task checks it on macOS with the traffic lights
  over the navbar and the name clear of them. Windows and Linux are checked for the absence of the
  effect - native decorations, no 86px gap - which `platform()` alone determines and which a
  stubbed unit test can pin without the platform.
- **`data-tauri-drag-region` swallows clicks on anything inside it that Tauri treats as draggable.**
  The attribute applies to the element and its children unless a child stops the event, so a button in
  the navbar can end up dragging the window instead of firing. → The attribute goes on the navbar's
  own container and the interactive children sit in their own elements; the verification task clicks
  Settings and About on macOS specifically to confirm they fire rather than drag.
- **Everything visible in this slice is disabled or inert.** A reviewer running it can click almost
  nothing, and a control that silently does nothing is indistinguishable from one that is broken. →
  Accepted, and bounded: the spec says which controls are disabled and why, and the Non-Goals list
  names the slice each inert control is waiting for. The one live control, Autopilot, is live.
- **The drag-and-drop invitation is a sentence the application cannot yet honour.** → Accepted for
  one slice, and recorded in the Non-Goals and in the proposal's parity-gap list so it is picked up
  rather than forgotten. It is the next slice's first job.
- **A component test in jsdom proves structure, not layout.** None of the metrics above - 48px, 86px,
  176px, the dot grid - can fail a Vitest assertion, because jsdom does not lay anything out. → The
  tests assert what jsdom can see (which controls exist, which are disabled, which classes are
  applied), and the verification task is a real `pnpm tauri dev` run compared against screen 01. That
  split is deliberate rather than a gap: a test asserting a Tailwind class string would pass on a
  layout that is visibly wrong.

**Platforms.** Developed and verified on macOS, which is also the only platform where the title-bar
decision has a visible effect. Windows and Linux are untested in this slice; what they exercise is
the absence of the macOS branch, and the risk there is that `titleBarStyle`/`hiddenTitle` are not as
inert as Tauri documents them to be. No execution provider is involved - this change runs no model
and touches no session.
