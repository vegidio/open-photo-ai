## Why

The GUI's frontend is still the scaffolding slice's placeholder: `App.tsx` renders one string to
prove the `opai::version()` -> Tauri command -> `invoke()` -> DOM path works, and
`crates/gui/package.json` carries React, React DOM and `@tauri-apps/api` and nothing else. There is
no styling system, no component library, no icon set, no translation catalogue and no client state.

The screens this project is building towards are designed and settled - 23 of them, in the Claude
Design project the Stack section names, built on shadcn/ui over zinc-dark surfaces with `#009aff`
kept as the primary. Every one of those screens needs the same foundation, and building it inside
whichever screen happens to land first would bury a decision that belongs to all of them. This
change installs that foundation on its own, so the first screen is a screen and not a screen plus a
toolchain.

## What Changes

- Tailwind CSS 4 is installed and wired into Vite through `@tailwindcss/vite`.
- `frontend/style.css` gains the palette as shadcn CSS variables: the zinc-dark surface and content
  scale, `--radius: 0.5rem`, and `#009aff` / `#007eff` kept as `--primary` and its hover. The
  `color-scheme: dark` pin, the `@layer base` `user-select: none` body rule with its mandatory
  `input, textarea` exception, and the scrollbar custom properties carry over from the Wails app,
  where each has a reason recorded beside it.
- shadcn/ui is set up (`components.json`, `lib/utils.ts` with `cn`) and the primitives the first
  screens need are generated: Button, Switch, Separator, Checkbox, Slider, ToggleGroup, Tooltip.
- `lucide-react` replaces the Wails app's `react-icons`, at stroke 1.5.
- Geist and Geist Mono are vendored through `@fontsource-variable`. They are **not** linked from
  Google Fonts: `tauri.conf.json` pins `font-src 'self' data:` and `style-src 'self'
  'unsafe-inline'` with no remote origin, so the CDN link the design file uses would be blocked at
  runtime. Self-hosting is the only form that works, and it is the better one for a desktop app
  that must render identically offline.
- i18next and react-i18next are installed and initialised, and all 13 locale catalogues
  (`en`, `de`, `el`, `es`, `fr`, `hi`, `id`, `ja`, `nl`, `pt`, `ru`, `sv`, `zh`) are ported verbatim
  from `../open-photo-ai/cmd/gui/frontend/src/i18n/locales/`. They are already written and already
  translated; taking them whole now means no later slice has to backfill twelve files to add one
  key.
- Zustand is installed. No store is created here - a store with no screen to serve is a store
  written against a guess.
- Nothing user-visible changes. The placeholder `App.tsx` still renders, unstyled, and is deleted by
  the next slice.

## Capabilities

### New Capabilities

None. This change installs frontend tooling and assets; it adds no behaviour a user or a downstream
system can observe. `.openspec.yaml` sets `skip_specs: true` accordingly.

### Modified Capabilities

None.

## Impact

**Crates touched: `gui` only**, and within it only `crates/gui/frontend/`, `crates/gui/package.json`
and the Biome/Vite/TypeScript configuration beside them. No Rust file changes, so `opai` and `cli`
are untouched. Nothing here is logic - it is a styling system, a component library, a font, an icon
set and a translation catalogue - so the "if both front ends would need it, it belongs in `opai`"
test does not apply: `cli` renders to a terminal and could not consume any of it.

**rust-sak: nothing applies.** Its modules cover hashing, HTTP, filesystem work, image codecs,
memoization, telemetry and hardware probes - all Rust, all backend. This change adds no Rust code at
all. There is no infrastructure here for it to already provide, and nothing in it is a candidate for
extending it.

**New frontend dependencies:** `tailwindcss` and `@tailwindcss/vite`; `class-variance-authority`,
`clsx` and `tailwind-merge` (what shadcn's generated components import); the Radix primitives those
components wrap; `lucide-react`; `@fontsource-variable/geist` and `@fontsource-variable/geist-mono`;
`zustand`; `i18next` and `react-i18next`. No new Rust dependencies.

**Deliberate divergence from the Go app's behaviour:** none. The catalogues are copied verbatim, so
every string is identical in all 13 languages; the two screens the design reworks (the first-run
download dialogs, which need new keys) are not in this slice. The changes to font, icon set and
palette are appearance, which the Stack section now settles, and the design is the source of truth
for it.

**Task estimate: 7**, comfortably inside the 15-task limit. This change is slice 1 of a series;
`design.md` carries the full slice list.
