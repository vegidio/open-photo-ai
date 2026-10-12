## 1. Wire (Rust)

- [x] 1.1 Add the `ColorBalance { codename, precision, bias }` arm to `Requested` in `crates/gui/src/enhance/operation.rs`, resolved through `VariantEntry::published(Family::ColorBalance, ..).build(..)` with `with_bias(Bias::clamped(bias))` and a `tracing::warn!` outside `Bias::MIN..=Bias::MAX` (design D1); update the module and enum docs that count three families. Verify with `cargo check -p gui`.
- [x] 1.2 Add tests beside the light-adjustment ones: every published colour-balance codename resolves at every published precision to `ColorBalance::rio`/`::saopaulo` at the given bias; a bias of 1.5 and of -2.0 resolves to `MAX`/`MIN`; an unpublished codename is refused as `UnknownOperation::Model { family: ColorBalance, .. }`. Verify with `cargo test -p gui enhance::operation`.
- [x] 1.3 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify all three pass clean.

## 2. Frontend model

- [x] 2.1 Add the `color_balance` member (`codename`, `precision`, `bias` in unit space, documented) to `Operation` in `ipc/enhance.ts`, add it to `RUNNABLE` and update that doc's family count and order note, add the `color_balance` arm to `newOperation` using `DEFAULT_BIAS`, and widen `DEFAULT_BIAS`'s doc to both bias families (design D5). Verify with `pnpm tsc --noEmit` passing, which also proves `Unoffered` is satisfied.
- [x] 2.2 Extend `lib/enhancements.test.ts`: `RUNNABLE_ENHANCEMENTS` is face recovery, light adjustment, colour balance, upscale in that order; `newOperation("color_balance", ..)` carries Rio (the catalogue's first model) and a bias of 0.5. Verify with `pnpm vitest run lib/enhancements`.

## 3. Frontend controls

- [x] 3.1 Build `features/enhancements/ColorBalanceOptions.tsx` matching `LightAdjustmentOptions`: `ModelTray`, `Separator`, `IntensityControl` labelled `t("enhancements.bias")`, bounds from the selected variant's `bias` parameter times 100, percent/unit conversion both ways, a chosen model keeping the bias; with a comment recording that it is a deliberate copy of `LightAdjustmentOptions` (design D3). Verify with `pnpm tsc --noEmit`.
- [x] 3.2 Wire `ColorBalanceOptions` and the `Rio, 50%, HD` info line (`enhancements.info`, `Math.round(bias * 100)`) into `EnhancementRow`'s two switches. Verify with `pnpm tsc --noEmit` and by the tests in 3.4.
- [x] 3.3 Add `ColorBalanceOptions.test.tsx` covering the spec's scenarios: the control is labelled Bias; typing 25 writes 0.25; 150 clamps to the published maximum; a slider commit writes once; choosing São Paulo keeps the bias; opening the panel writes nothing. Verify with `pnpm vitest run features/enhancements/ColorBalanceOptions`.
- [x] 3.4 Extend `AddEnhancement.test.tsx` and `EnhancementList.test.tsx` (and `test/support.tsx`'s catalogue fixture if it lacks colour balance): the menu offers Color Balance between Light Adjustment and Upscale; adding it leaves its panel closed; its row reads `Rio, 50%, HD`, and `-40%` for a bias of -0.4. Verify with `pnpm vitest run features/enhancements`.
- [x] 3.5 Run biome scoped to the files this change touched (not `pnpm check` over the repo), then the whole frontend suite, and verify both pass.

## 4. Verification

- [x] 4.1 Hand a launch check to the user rather than driving the GUI: open `cmd/perf/test.dat`, add Color Balance, confirm the row, the panel matching Light Adjustment's, a negative bias moving the colours the opposite way to a positive one, and a Rio/São Paulo switch each re-running once. Record the outcome here.
  - **Outcome (2026-09-23):** checked by the user on macOS and passed on every point: the `Rio, 50%, HD` row, the panel matching Light Adjustment's, a negative bias moving the colours the opposite way to a positive one, and each Rio/São Paulo switch re-running once. The same check covered the 8px gap between the slider and its 0 mark, added to `IntensityControl` for both bias panels.
- [x] 4.2 Run `openspec validate add-gui-color-balance --strict` and verify it passes.
