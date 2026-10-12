## 1. Store

- [x] 1.1 Add `tensorrtAsked: boolean` to the settings store's state outside `SettingsData`, persist it through
      `partialize` beside `settingsData(state)`, and repair it in `merge` as `stored.tensorrtAsked === true ||
      stored.isFirstTensorRT === false`; verify tests assert that a first launch reads `false`, that a stored `true`
      survives, that a Wails blob with `isFirstTensorRT: false` reads `true` and one with `true` reads `false`, and that
      `settingsData()` and `settingsDefaults()` do not carry it
- [x] 1.2 Add `answerTensorRT(enable: boolean)`, setting `processor` to `"auto"` or `"cuda"` and `tensorrtAsked` to
      `true` in one `set`, and update the store's documentation of `apply` as the only writer to name it; verify tests
      assert both answers' processor, the flag, that what was persisted carries both, and that a later Settings Save
      and a Performance-page reset leave the flag `true`

## 2. The dialog

- [x] 2.1 Copy `logo_tensorrt.avif` from `../open-photo-ai/cmd/gui/frontend/src/assets/` into the frontend and import
      it; verify `pnpm typecheck` passes and `pnpm build` emits the asset
- [x] 2.2 Build `TensorRTDialog` on the shadcn `dialog` to screen 03's layout (see design.md), reading the existing
      `dialogs.tensorRT.*`, `common.yes` and `common.no` keys - no new i18n keys, so no locale edits - with the
      emphasis through `<Trans>` `components`, No calling `answerTensorRT(false)` and Yes `answerTensorRT(true)`, and
      Escape and outside clicks prevented; verify tests assert the title, the three paragraphs and the footnote
      render, each button's resulting processor and flag, and that Escape and an outside click leave it open
- [x] 2.3 Render it from `App` when `status === "ready" && providers?.tensorrt && !tensorrtAsked`; verify `App` tests
      assert it appears after a successful setup reporting TensorRT, does not appear without TensorRT, after an
      earlier answer, while the setup dialog or the failure dialog is up, and does appear after a retry that
      succeeds - and goes away once answered

## 3. Checks

- [x] 3.1 Run `pnpm typecheck`, the Vitest suite, and biome scoped to the files this change touched; verify all three
      are clean

## 4. Fake data (temporary - remove before archiving)

- [x] 4.1 Widen the report passed to `succeeded` to `{ ...providers, cuda: true, tensorrt: true }` and drop
      `!tensorrtAsked` from the dialog condition, each line marked `// FAKE(tensorrt-prompt)`; verify by launching the
      app on macOS that the dialog appears after setup on every launch, that No shows CUDA selected on the
      Performance page and Yes shows Auto, and screenshot it for the user to review
- [x] 4.2 After the user has reviewed it, remove every `FAKE(tensorrt-prompt)` line; verify `grep -rn
      "FAKE(tensorrt-prompt)" crates/gui/frontend` returns nothing and the Vitest suite still passes
