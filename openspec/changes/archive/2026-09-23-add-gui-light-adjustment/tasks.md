## 1. Wire (Rust)

- [x] 1.1 Add the `LightAdjustment { codename, precision, bias }` arm to `Requested` in `crates/gui/src/enhance/operation.rs`, resolved through `VariantEntry::published(Family::LightAdjustment, ..).build(..)` with `with_bias(Bias::clamped(bias))` and a `tracing::warn!` outside `Bias::MIN..=Bias::MAX` (design D1); update the module and enum docs that say two families. Verify with `cargo check -p gui`.
- [x] 1.2 Add tests beside the upscale and face-recovery ones: every published light-adjustment codename resolves at every published precision to `LightAdjustment::paris`/`::lyon` at the given bias; a bias of 1.5 and of -2.0 resolves to `MAX`/`MIN`; `NaN` is unreachable from JSON so is not tested; an unpublished codename is refused as `UnknownOperation::Model`. Verify with `cargo test -p gui enhance::operation`.
- [x] 1.3 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify all three pass clean.

## 2. Frontend model

- [x] 2.1 Add the `light_adjustment` member (`codename`, `precision`, `bias` in unit space, documented) to `Operation` in `ipc/enhance.ts`, add it to `RUNNABLE`, update the `RUNNABLE` doc's family count, and add the `light_adjustment` arm to `newOperation` with `DEFAULT_BIAS = 0.5` (design D5). Verify with `pnpm tsc --noEmit` passing, which also proves `Unoffered` is satisfied.
- [x] 2.2 Extend `lib/enhancements.test.ts`: `RUNNABLE_ENHANCEMENTS` is face recovery, light adjustment, upscale in that order; `newOperation("light_adjustment", ..)` carries the default model and a bias of 0.5. Verify with `pnpm vitest run lib/enhancements`.

## 3. Frontend controls

- [x] 3.1 Add the `enhancements.bias` key to all thirteen `i18n/locales/*.json`, translated per locale, and verify with the existing locale-parity test (or `pnpm vitest run i18n` if that is where it lives) that every catalogue has the key.
- [x] 3.2 Build `features/enhancements/IntensityControl.tsx` per design D2 - label, typed percentage with `%` suffix at a third of the row, slider with `track={false}`, step 5, a labelled mark beneath the track, local drag state shown in the field, write on `onValueCommit`, `""`/`"-"` held and restored on blur, typed values clamped, slider disabled without bounds - matching screen 11b's metrics. Verify by building it into task 3.3's panel.
- [x] 3.3 Build `features/enhancements/LightAdjustmentOptions.tsx`: `ModelTray`, `Separator`, `IntensityControl` labelled `t("enhancements.bias")`, bounds from the selected variant's `bias` parameter times 100 (design D4), percent/unit conversion both ways. Wire it and the `Paris, 25%, HD` info line (`enhancements.info`, `Math.round(bias * 100)`) into `EnhancementRow`'s two switches. Verify with `pnpm tsc --noEmit`.
- [x] 3.4 Add `IntensityControl.test.tsx` and `LightAdjustmentOptions.test.tsx` covering the spec's scenarios: typing 25 writes 0.25; `-` writes nothing and `-30` writes -0.3; 150 clamps to the published maximum; an emptied field restores on blur; a slider commit writes once; choosing Lyon keeps the bias; opening the panel writes nothing. Verify with `pnpm vitest run features/enhancements`.
- [x] 3.5 Extend `AddEnhancement.test.tsx` and `EnhancementList.test.tsx`: the menu offers Light Adjustment between Face Recovery and Upscale; adding it leaves its panel closed; its row reads `Paris, 50%, HD`, and `-40%` for a bias of -0.4. Verify with `pnpm vitest run features/enhancements`.
- [x] 3.6 Run biome scoped to the files this change touched (not `pnpm check` over the repo), then the whole frontend suite, and verify both pass.

## 4. Verification

- [x] 4.1 Hand a launch check to the user rather than driving the GUI: open `cmd/perf/test.dat`, add Light Adjustment, confirm the row, the panel against screen 11b, a negative bias moving the photograph the opposite way to a positive one, and a Paris/Lyon switch each re-running once. Record the outcome here. Outcome (2026-09-23): confirmed by the user - the row, the panel, a negative bias reversing the adjustment and a Paris/Lyon switch each re-running once all work. The gap between the Bias row and the slider was widened by 8px (12px to 20px) on their review.
- [x] 4.2 Run `openspec validate add-gui-light-adjustment --strict` and verify it passes.
