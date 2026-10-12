## 1. The preference

- [x] 1.1 Add `background` to `SettingsData` in `stores/settings.ts` - a `"particles" | "dotted"` union
      with a `PROCESSORS`-style exported list and a guard, defaulting to `"particles"`, with its setter.
      Verify `pnpm typecheck` fails until `"background"` is added to `SNAPSHOT_KEYS`, which is the
      `Uncovered` alias doing its job, and passes once it is.
- [x] 1.2 Repair a stored `background` in the store's `merge`, the way `processor` is repaired: a value
      naming neither surface reads as `"particles"`. Verify with store tests covering an absent field (a
      `settings-storage` written by the Wails application), a value naming neither surface, and each of
      the two valid values round-tripping.
- [x] 1.3 Verify by test that the setting is a draft: changing it and calling `cancelDraft` restores it,
      `commitDraft` persists it, and nothing reaches `localStorage` while a draft is open.

## 2. The settings row

- [x] 2.1 Add shadcn `radio-group`, then apply the four standing fixes - repoint the `cn` import, make
      the React import named, convert the components to arrow functions, and remove the bogus `cn`
      package the generator adds to `package.json`. Verify `pnpm lint` and `pnpm typecheck` pass.
- [x] 2.2 Add `RadioRow` to `features/settings/rows.tsx` as the fifth shape on the existing `SettingsRow`
      frame, with the design's two-column grid inside the shared 196px control column. Verify by test
      that the group is addressable by the row's label, that exactly one option is checked, and that
      choosing the other reports it once.
- [x] 2.3 Add the `BackgroundRow` to `features/settings/ApplicationRows.tsx` and register `app_background`
      in `sections.ts` between `app_language` and `app_logs`. Verify `SettingsDialog.test.tsx`'s existing
      walk still passes - every entry leads to a row and every row is named in the list - with the new
      row included.
- [x] 2.4 Add `settings.app.background.{title,description,particles,dotted}` to all thirteen locale files,
      not only `en.json`. Verify `i18n/parity.test.ts` passes.

## 3. The particle field

- [x] 3.1 Vendor MagicUI's `particles` into `components/ui/particles.tsx` with the three adaptations from
      design.md D3: no mouse magnetism and no `MousePosition` hook, `useResizeObserver` in place of the
      debounced window listener, and no `staticity` prop. Verify by test that it mounts a canvas, that it
      registers no `mousemove` listener on `window`, and that it is `aria-hidden` and
      `pointer-events-none`.
- [x] 3.2 Gate the animation loop on window focus per D4, via `getCurrentWindow().onFocusChanged` with a
      DOM `focus`/`blur` fallback for the test environment. Verify by test that losing focus cancels the
      pending frame, that the canvas is not cleared, and that regaining focus schedules a new frame
      without re-initialising the particle array.

## 4. The canvas surface

- [x] 4.1 Replace `DOTTED_CANVAS` in `lib/constants.ts` with the single surface seam of D1, read by the
      preview now and by the crop dialog later, carrying the design's `quantity=120`, `color=#ffffff`,
      `size=0.6`. Verify by test that it yields the dotted classes for one choice and the particle canvas
      for the other, and that `bg-background` is present in both.
- [x] 4.2 Apply it in `features/preview/Preview.tsx`, with the canvas's children lifted above the particle
      canvas as the design lifts them (`relative z-10`). Verify by test that the empty state's invitation
      and the loaded image are both rendered above the field, and that the canvas draws the chosen
      surface in both states.

## 5. Verification

- [x] 5.1 Verify by test that a saved choice changes the canvas without a remount, and that a choice made
      and cancelled leaves the canvas drawing the surface it had throughout - the applies-on-save rule of
      design.md D6.
- [x] 5.2 Run the application and confirm by eye: particles on a fresh profile, the dotted grid after
      choosing it, the field drifting without reacting to the pointer, the wheel still zooming and drag
      still panning the canvas with the field drawn, and the animation stopping when the window is sent
      behind another and resuming when it is brought back.

      Confirmed in the running application: the particle field on a profile with no stored background,
      the dotted grid after choosing it and saving, the choice surviving a restart, the canvas keeping
      its surface while the choice was still a draft, the wheel still zooming and drag still panning
      with the field drawn, and the animation gate - 0 frames in 3s while the window was visible but
      unfocused, against 180 in 3s while focused, with the field still drawn throughout. The one leg
      not confirmed by eye is that the field ignores the pointer, accepted as covered structurally by
      `components/ui/particles.test.tsx`: no `mousemove` listener on `window`, and
      `pointer-events-none` on the element over the canvas's own handlers.
- [x] 5.3 Run `pnpm lint`, `pnpm typecheck` and `pnpm test`, scoping the Biome pass to the files this
      change touched so the diff does not swell with unrelated reformatting.
