## 1. Wire (Rust)

- [x] 1.1 Add the `Denoise { codename, precision, strength }` arm to `Requested` in `crates/gui/src/enhance/operation.rs`, resolved through `VariantEntry::published(Family::Denoise, ..).build(..)` with `with_strength(Strength::clamped(strength))` and a `tracing::warn!` outside `Strength::MIN..=Strength::MAX` (design D1); update the module and enum docs that count four families. Verify with `cargo check -p gui`.
- [x] 1.2 Add tests beside the colour-balance ones: every published denoise codename resolves at every published precision to `Denoise::stockholm`/`::gothenburg`/`::malmo` at the given strength; a strength of 4.0 and of -1.0 resolves to `MAX`/`MIN`; an unpublished codename (`rio`) is refused as `UnknownOperation::Model { family: Denoise, .. }`. Verify with `cargo test -p gui enhance::operation`.
- [x] 1.3 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify all three pass clean.

## 2. Frontend model and i18n

- [x] 2.1 Add the `denoise` member (`codename`, `precision`, `strength` in unit space, documented) to `Operation` in `ipc/enhance.ts`; add `denoise` to the front of `RUNNABLE` and update its doc's family count and order note (design D5); add `DEFAULT_STRENGTH = 1` beside `DEFAULT_BIAS` and the `denoise` arm of `newOperation` (design D4). Verify with `pnpm tsc --noEmit` passing, which also proves `Unoffered` is satisfied.
- [x] 2.2 Extend `lib/enhancements.test.ts`: `RUNNABLE_ENHANCEMENTS` is denoise, face recovery, light adjustment, colour balance, upscale in that order; `newOperation("denoise", ..)` carries Stockholm (the catalogue's first model) and a strength of 1. Verify with `pnpm vitest run lib/enhancements`.
- [x] 2.3 Add `enhancements.strength` beside `enhancements.bias` in all thirteen locale files, each translation distinct from that locale's `enhancements.intensity` (design D3). Verify every locale file parses and carries the key (e.g. a `jq '.enhancements.strength'` over each returns a non-empty string) and the frontend's i18n tests pass.

## 3. Frontend controls

- [x] 3.1 Build `features/enhancements/IntensityOptions.tsx` (design D2): `ModelTray`, `Separator`, `IntensityControl`; props `enhancement`, `entry`, `codename`, `precision`, `parameter` (`"bias" | "strength"`), `label`, `mark`, `amount` (unit), `onModelChange(option)`, `onAmountChange(unit)`; the range read from the selected variant's parameter named `parameter`, times 100; the percent/unit conversion both ways. Verify with `pnpm tsc --noEmit`.
- [x] 3.2 Reduce `LightAdjustmentOptions` and `ColorBalanceOptions` to wrappers over `IntensityOptions` (parameter `bias`, label `enhancements.bias`, mark 0, field `bias`), keeping their props; remove `ColorBalanceOptions`' "deliberate copy" comment and point both docs at design D2. Verify with `pnpm vitest run features/enhancements/LightAdjustmentOptions features/enhancements/ColorBalanceOptions` passing **with those test files unchanged**.
- [x] 3.3 Build `features/enhancements/DenoiseOptions.tsx` as a wrapper (parameter `strength`, label `enhancements.strength`, mark 100, field `strength`), and wire it and the `Stockholm, 100%, HD` info line (`enhancements.info`, `Math.round(strength * 100)`) into `EnhancementRow`'s two switches (design D6). Verify with `pnpm tsc --noEmit` and by the tests in 3.4 and 3.5.
- [x] 3.4 Add `DenoiseOptions.test.tsx` covering the spec's scenarios: the control is labelled Strength; typing 150 writes 1.5; 400 clamps to the published maximum; -20 writes 0; a slider commit writes once; choosing Malmö keeps the strength; opening the panel writes nothing. Verify with `pnpm vitest run features/enhancements/DenoiseOptions`.
- [x] 3.5 Extend `AddEnhancement.test.tsx` and `EnhancementList.test.tsx` (and `test/support.tsx`'s catalogue fixture if its denoise entry lacks the `strength` parameter): the menu offers Denoise first; adding it leaves its panel closed; its row reads `Stockholm, 100%, HD`, and `250%` for a strength of 2.5; a denoise added after a face recovery is listed above it. Verify with `pnpm vitest run features/enhancements`.
- [x] 3.6 Run biome scoped to the files this change touched (not `pnpm check` over the repo), then the whole frontend suite, and verify both pass.

## 4. Verification

- [x] 4.1 Hand a launch check to the user rather than driving the GUI: open `cmd/perf/test.dat`, add Denoise, confirm the `Stockholm, 100%, HD` row, the panel matching screen 12 (Strength label, 0..300 slider, `100` under the thumb at 100%), a strength of 0 giving back the photograph and 200 denoising harder than 100, each Stockholm/Gothenburg/Malmö switch re-running once, and the Light Adjustment and Color Balance panels looking and behaving as before. Record the outcome here. Passed (confirmed by the user, 2026-09-24).
- [x] 4.2 Run `openspec validate add-gui-denoise --strict` and verify it passes.
