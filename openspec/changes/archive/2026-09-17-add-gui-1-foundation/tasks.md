## 1. Styling foundation

- [x] 1.1 Add the frontend dependencies to `crates/gui/package.json` - `tailwindcss` and
  `@tailwindcss/vite`; `class-variance-authority`, `clsx` and `tailwind-merge`; `lucide-react`;
  `@fontsource-variable/geist` and `@fontsource-variable/geist-mono`; `zustand`; `i18next` and
  `react-i18next` - and verify `pnpm install` succeeds and `pnpm typecheck` still passes against the
  untouched placeholder.

- [x] 1.2 Wire Tailwind 4 into Vite through `@tailwindcss/vite` and write `frontend/style.css`: the
  shadcn zinc-dark variables taken from the design project's `shadcn-mapping.md` table with
  `#009aff` / `#007eff` kept as `--primary` and its hover, `--radius: 0.5rem`, the
  `color-scheme: dark` pin, the `@layer base` `user-select: none` body rule with its
  `input, textarea` exception, and the `.scrollbar-thin` custom properties. Verify by importing the
  sheet from `main.tsx` and confirming `pnpm build` emits both a Tailwind utility class and the
  custom properties, and that the placeholder renders on the `#09090b` background.

- [x] 1.3 Import `@fontsource-variable/geist` and `@fontsource-variable/geist-mono` in `main.tsx` and
  set them as the sans and mono families in `style.css`. Verify that `grep -r 'fonts.googleapis.com\|fonts.gstatic.com' dist/`
  finds nothing after `pnpm build` - the CSP would block a remote font at runtime with no build-time
  error - and that the window renders in Geist under `pnpm tauri dev` with a clean console.

## 2. Component library

- [x] 2.1 Create `components.json` with every alias pointed explicitly at `frontend/` (matching the
  existing `@/*` mapping) and `frontend/lib/utils.ts` exporting `cn`, then generate Button, Switch,
  Separator, Checkbox, Slider, ToggleGroup and Tooltip. Verify each landed in
  `frontend/components/ui/` and not in a `src/` the CLI invented, run `pnpm check` to bring them to
  the project's Biome style, and confirm `pnpm typecheck` passes against the strict `tsconfig.json`
  (`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `verbatimModuleSyntax`).

## 3. Translations

- [x] 3.1 Copy all 13 locale catalogues verbatim from
  `../open-photo-ai/cmd/gui/frontend/src/i18n/locales/` (`en`, `de`, `el`, `es`, `fr`, `hi`, `id`,
  `ja`, `nl`, `pt`, `ru`, `sv`, `zh`) into `frontend/i18n/locales/`, and add `frontend/i18n/index.ts`
  initialising i18next with them, fixed to `en` and with no detector - persistence belongs to the
  settings slice. Verify with a test that resolves `preview.empty.browse` to `Browse images`, and
  confirm no `t()` call exists at module scope anywhere.

- [x] 3.2 Add a test asserting every catalogue carries exactly the same key set as `en.json`,
  reporting missing and extra keys by dotted path. Verify it passes on the 13 copied files and fails
  when a key is deleted from one of them.

## 4. Verification

- [x] 4.1 Run `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm build` in `crates/gui`, and
  `cargo fmt --check`, `cargo clippy` and `cargo test` at the workspace root as a regression guard -
  no Rust changed in this slice, so all three must pass untouched. Launch `pnpm tauri dev` and
  confirm the placeholder still renders, now styled, with no console error and no blocked request in
  the devtools network panel.
