## Context

See proposal.md - Why. What matters here is the starting point: `crates/gui/frontend/` holds three
TypeScript files and a test, `package.json` lists React, React DOM and `@tauri-apps/api`, and
`tsconfig.json` already maps `@/*` to `./frontend/*` with a comment explaining that the alias exists
so ported Wails components transfer unchanged. Vite mirrors that alias at runtime.

Two constraints shape most of what follows.

**The CSP is already written and already strict.** `tauri.conf.json` pins `font-src 'self' data:`,
`style-src 'self' 'unsafe-inline'` and `connect-src 'self' ipc: http://ipc.localhost`, in both the
release and the dev policy. No remote origin is allowed in any of them. The design file loads Geist
from `fonts.googleapis.com`; that link cannot work here, and relaxing the policy to let it work is
not on the table for a desktop application that has no reason to reach the network to draw its own
text.

**The catalogues already exist, in 13 languages.** `../open-photo-ai/cmd/gui/frontend/src/i18n/locales/`
holds `en`, `de`, `el`, `es`, `fr`, `hi`, `id`, `ja`, `nl`, `pt`, `ru`, `sv` and `zh`, all written
and all translated, at commit `a024eae` (2026-09-13). Any screen ported here uses keys that are
already in them.

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | **CURRENT** |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs. Placeholder `App.tsx` deleted | 1 | pending |

**Slice 1 is this change.** It touches `gui` only, and within `gui` only the frontend. `opai` and
`cli` are untouched; see proposal.md - Impact for why none of this is shared logic.

**rust-sak: no feature is used, and none applies.** This change adds no Rust code. Nothing in it is
a candidate for extending rust-sak.

**No new Rust crates.** The new dependencies are all npm packages and are listed in
proposal.md - Impact.

## Goals / Non-Goals

**Goals:**

- A frontend that any of the 23 designed screens can be built on without first adding tooling.
- Token names that let `shadcn add <component>` keep working for the rest of the port.
- Identical rendering offline and on every platform, including the type.
- The 13 catalogues in the repository, complete, so adding a key is one edit to 13 files that are
  already there rather than a port on the side.

**Non-Goals:**

- Any visible change. The placeholder still renders, now unstyled against a dark background; slice 2
  deletes it.
- Any Zustand store. Stores are shaped by the screens that read them; one written now is written
  against a guess.
- Language detection or persistence. The Wails app reads the persisted language from its settings
  store, and there is no settings store here yet. Slice 1 initialises i18next to `en`; the settings
  slice adds the persistence and the switch.
- Every shadcn primitive. Only the ones screens 01 and 02 need are generated; the rest arrive with
  the screens that use them, which is how `shadcn add` is meant to be used.
- The macOS overlay title bar and the platform IPC behind the 86px inset. Both belong to the navbar
  that consumes them, and both are user-visible - setting the title bar here would leave the
  placeholder string floating under detached traffic lights for the length of one slice. They are
  slice 2's.

## Decisions

### Geist is vendored, not linked

`@fontsource-variable/geist` and `@fontsource-variable/geist-mono`, imported from
`frontend/main.tsx` the way the Wails app imports Roboto.

*Alternatives considered.* Linking the Google Fonts stylesheet, as the design file does - blocked by
`font-src`/`style-src`, and would make the application's typography depend on a network it otherwise
never touches. Adding the CDN to the CSP - a permanent widening of the policy to save vendoring two
font files, and a remote origin in `style-src` is worth more than that. Keeping Roboto - the design
is the source of truth for appearance and it specifies Geist.

The font files are the only part of this change that could silently fail at runtime rather than at
build time, which is why the verification task loads the app and confirms the rendered family.

### Tokens use shadcn's semantic names, not the Wails app's role names

The Wails app's `@theme` block defines `--color-surface-base`, `--color-surface-raised`,
`--color-content-primary` and so on - good names, and the reasoning beside them is sound. They are
not carried over. shadcn's generated components reference `--background`, `--card`, `--popover`,
`--primary`, `--muted-foreground`, `--border`, `--input` and `--ring` by those names, in the class
strings the CLI writes. Renaming the token set means hand-editing every generated component and
re-editing each one after every `shadcn add` - which ends with the CLI unusable and the component
library hand-maintained.

The mapping from the old names to the new ones is written down in the design project's
`shadcn-mapping.md`; it is one table and it is the authority for the values.

*Trade-off accepted.* `--card` is one value where the Wails palette had three surface levels, so a
distinction the old tokens could express is flattened. The design already made that call - the
mapping table maps both `--color-surface` and `--color-surface-raised` onto `#18181b` - so this
follows it rather than inventing a fourth scheme.

### Tailwind 4, CSS-first

`@import "tailwindcss"` and `@theme` in `frontend/style.css`, wired through `@tailwindcss/vite`. No
`tailwind.config.js`. This is Tailwind 4's own model, it is what the Wails app already uses, and it
is what current shadcn expects.

### shadcn components are generated into the repository and formatted with ours

`components.json` points its aliases at `frontend/` (matching the existing `@/*` mapping), so
components land in `frontend/components/ui/` and `cn` in `frontend/lib/utils.ts`. They are our
source from the moment they are written - not a dependency - and they are formatted and linted with
the project's Biome configuration rather than excluded from it.

*Consequence to remember:* re-running `shadcn add` writes CLI-formatted files, so it is followed by
`pnpm check`. That is cheaper than an ignore rule that would let a generated file drift from the
house style permanently. `pnpm check` does not finish the job on its own, though - the CLI's output
breaks three of the project's frontend rules that Biome has no rule for, so every `shadcn add` is
followed by the same three hand-edits:

1. `function Button(...)` becomes `const Button = (...) =>`. Biome's `useArrowFunction` only rewrites
   function *expressions*, so a declaration passes the linter untouched.
2. `import * as React from "react"` becomes a named import (`ComponentProps`, `CSSProperties`,
   `createContext`, ...), and the `React.` qualifiers go with it.
3. The `cn` import is rewritten back to `@/lib/utils`, and the `cn` package the CLI installs
   alongside it is removed from `package.json` - it is not the one the components mean.

Generated now: **Button, Switch, Separator, Checkbox, Slider, ToggleGroup, Tooltip** - the set
screen 01 needs (Button for Browse images / Add images / Settings / About / Add enhancement /
Export, Switch for Autopilot, Separator for the navbar and drawer dividers, Checkbox for Select all,
Slider for zoom, ToggleGroup for the preview-mode tray, Tooltip for its labels).

### Icons are set to stroke 1.5 once, in a provider

`main.tsx` wraps the tree in lucide's own `<LucideProvider strokeWidth={1.5}>`. Stroke 1.5 is the
weight the design draws icons at; lucide's default is 2.

*Alternatives considered.* `strokeWidth={1.5}` at each call site - the shadcn components render
icons of their own (Checkbox's tick today, more with every `shadcn add`), so the prop would have to
be added to generated files after every run and would still be a rule to remember in every screen.
A CSS rule on `svg.lucide` - it would work, and it would put an appearance decision somewhere no
one reading a component would look for it, in a stylesheet whose job is the palette.

Radix's portals are portals in the DOM but children in the React tree, so the icons inside tooltips
and popovers mounted on `document.body` inherit the context too.

### All 13 catalogues come across whole, verbatim

Copied from `../open-photo-ai/cmd/gui/frontend/src/i18n/locales/` at commit `a024eae`, unedited.

*Alternatives considered.* Trimming each to the keys screen 01 uses - a smaller diff now, and 13
files re-opened by every later slice, which is exactly the shape of work that gets skipped. English
only - breaks the project rule that a key lands in every existing locale file, and turns twelve
translations that already exist into work to be redone.

The catalogues are a snapshot of another repository, so they can drift from it. The commit is
recorded above so a later re-sync has a base to diff against.

`t()` is never called at module scope - the Wails app records why: it would capture the language at
evaluation time and never update. That rule carries over with the catalogues.

### Slice 1's test is the catalogue invariant

Nothing renders in this slice, so there is no component test to write. What there is to verify is
the one property 13 parallel JSON files lose first: a test asserts that every catalogue has exactly
the same key set as `en.json`, reporting the missing and extra keys by path. It is worth having on
its own account - it is the check that makes "add the key to every locale" enforceable rather than
remembered - and it means this change verifies something real rather than only compiling.

## Risks / Trade-offs

- **The shadcn CLI assumes a `src/` layout and may write outside `frontend/`.** → `components.json`
  sets every alias explicitly against `frontend/` before the first `shadcn add`, and the first
  generated component's path is checked rather than assumed.
- **Version skew across Tailwind 4, Vite 8, React 19 and TypeScript 7.** This project is on current
  majors of all four, which is ahead of most shadcn documentation. → The primitives are generated and
  typechecked in this slice, where a failure is a tooling problem with nothing else in flight, rather
  than discovered mid-screen in slice 2.
- **A remote `@import` could reach a stylesheet later and be blocked only at runtime.** → The font
  decision above is recorded where the next person adding a typeface will read it, and the
  verification task confirms the rendered family rather than trusting the import.
- **The catalogues drift from the Go repository.** → Accepted; the source commit is recorded so the
  drift is measurable.
- **`tsconfig.json` is strict in ways generated code often is not** - `noUncheckedIndexedAccess`,
  `exactOptionalPropertyTypes`, `verbatimModuleSyntax`. → Generated components are typechecked as
  part of the task that generates them; where one needs an edit, the edit is ours to keep, which is
  the point of shadcn's copy-in model.

**Platforms.** The work is platform-independent - no Rust, no conditional compilation, no native
API. It will be developed and verified on macOS. Windows and Linux are untested in this slice;
nothing in it is expected to differ, and slice 2, which adds the first platform-conditional
behaviour (the macOS title bar inset), is where that stops being true.
