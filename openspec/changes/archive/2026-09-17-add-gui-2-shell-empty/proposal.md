## Why

Slice 1 installed the frontend foundation and stopped there: Tailwind, shadcn primitives, Geist,
lucide, thirteen catalogues and Zustand are all in place, and what the window still renders is
`App.tsx`'s placeholder - the string `Open Photo AI 26.9.0` on a dark background. Nothing of the
application's own shape exists yet.

Screen 01 in the Claude Design project is that shape at rest: the navbar, the dotted preview canvas
carrying the drag-and-drop invitation, the image drawer folded to its 48px header, and the sidebar
from "No preview available" down to the disabled Export button. Every screen after it is this frame
with something inside, so building it now means the slices that follow add a file list, a preview
image or a dialog rather than a frame plus their own feature. It is also the first change that has
to answer a platform question - on macOS the window's traffic lights sit **inside** the navbar, and
the inset that keeps them clear needs to know which platform it is on.

## What Changes

- **The macOS overlay title bar.** `tauri.conf.json`'s main window gains `"titleBarStyle": "Overlay"`,
  `"hiddenTitle": true` and `"trafficLightPosition"`, the last of which centres the window controls in
  the 48px navbar as screen 01 draws them - macOS otherwise places them for its own shorter title bar.
  All three are macOS-only keys, so Windows and Linux keep their native decorations, which is what the
  Wails app does (`MacTitleBarHidden` with no `Frameless` anywhere). The navbar carries
  `data-tauri-drag-region` so the window is still dragged by the strip that replaced its title bar.
- **A platform seam on the frontend.** `frontend/ipc/os.ts` reports the host platform, and the navbar
  uses it for the 86px left inset that clears the traffic lights on macOS and is absent everywhere
  else. This replaces the Wails `OsService.GetOS()` binding.
- **Four shell components**, under `frontend/features/`: `Navbar`, `Preview`, `Drawer` and `Sidebar`,
  composed by a rewritten `App.tsx`. The placeholder and its test are deleted.
- **The empty state is the only state they render.** There is no way to open a file yet, so every
  file-dependent control renders disabled exactly as screen 01 draws it: the drawer's fold toggle,
  Select all, the preview-mode tray and the zoom slider; Add enhancement; Export image. Settings,
  About and Browse images render enabled but inert - their dialogs and the file picker are later
  slices.
- **One Zustand store**, `frontend/stores/enhancements.ts`, holding the Autopilot flag and nothing
  else. It is the shell's only live control, it defaults on, and it persists - all three as in the
  Wails app, whose store `partialize`s down to exactly this field.
- **No new i18n keys.** Every string screen 01 shows is already in the thirteen catalogues slice 1
  copied across: `navbar.about`, `navbar.version`, `settings.title`, `preview.empty.title`,
  `preview.empty.browse`, `common.or`, `drawer.addImages`, `drawer.selectAll`, `drawer.preview.*`,
  `sidebar.autopilot`, `sidebar.noPreview`, `sidebar.exportImage` and `enhancements.add`.
- **No new shadcn primitives.** Slice 1 generated Button, Switch, Separator, Checkbox, Slider,
  ToggleGroup and Tooltip precisely because screen 01 needs that set.

## Capabilities

### New Capabilities

- `gui-shell`: the application window's persistent frame - what the navbar, the preview canvas, the
  image drawer and the sidebar show, which of their controls are live when no image is open, and how
  the window is titled and dragged on each platform.

### Modified Capabilities

None.

## Impact

**Crates touched: `gui` only.** The Rust side gains one plugin registration in `src/lib.rs`, one
permission in `capabilities/default.json` and three window keys in `tauri.conf.json`. Everything else
is under `crates/gui/frontend/`. `opai` and `cli` are untouched.

Nothing here is shared logic. It is a window frame, a set of React components and one boolean the
user toggles; `cli` renders to a terminal and could not consume any of it. The one fact that could
have been core - which platform the process runs on - is not, because Rust answers it with
`std::env::consts::OS` at compile time and `opai` would only be relaying a constant.

**rust-sak: nothing applies.** Its modules cover hashing, HTTP, filesystem work, image codecs,
memoization, telemetry and hardware probes. `sysinfo` is the nearest - it probes CPU, memory and GPU,
not which OS a build targets - and nothing here needs even that. There is no infrastructure in this
change for it to already provide and nothing in it worth extending it with.

**New dependencies.** Rust: `tauri-plugin-os` (v2, same release train as the Tauri core already
pinned). Frontend: `@tauri-apps/plugin-os`. No other crate and no other npm package.

**Deliberate divergence from the Wails app's behaviour:** none in this slice. The layout metrics,
the disabled states and the Autopilot default are the Wails app's, verified against
`cmd/gui/frontend/src/features/{navbar,preview,drawer,sidebar}` rather than assumed. Appearance
follows the design project, which the Stack section makes the authority for it - the palette, the
lucide icon set and the drawer's two-tone surfaces are appearance, not behaviour.

**Parity gaps this slice knowingly leaves open**, each belonging to a named later slice: the file
picker behind Browse images and Add images, actual drag-and-drop handling, the Settings and About
dialogs, the current-file name and Dimensions blocks in the navbar, and the update check behind
"Update Available".

**Task estimate: 12**, inside the 15-task limit. This change is slice 2 of a series; `design.md`
carries the full slice list.
