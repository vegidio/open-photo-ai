## 1. Wire (Rust)

- [x] 1.1 Add the `Sharpen { codename, precision, strength }` arm to `Requested` in `crates/gui/src/enhance/operation.rs`, resolved through `VariantEntry::published(Family::Sharpen, ..).build(..)` with `with_strength(Strength::clamped(strength))` and a `tracing::warn!` outside `Strength::MIN..=Strength::MAX` (design D1); update the module and enum docs that count five families. Verify with `cargo check -p gui`.
- [x] 1.2 Move `a_family_this_application_cannot_run_is_refused_by_the_parse_and_names_it` to a colorization request (`delhi`), keeping its assertion that the refusal names the family (design D2). Verify with `cargo test -p gui enhance::operation::tests::a_family_this_application_cannot_run`.
- [x] 1.3 Add tests beside the denoise ones: every published sharpen codename resolves at every published precision to `Sharpen::moscow`/`::stpetersburg`/`::novgorod` at the given strength (driven from the catalogue); a strength of 4.0 and of -1.0 resolves to `MAX`/`MIN`; an unpublished codename (`stockholm`) is refused as `UnknownOperation::Model { family: Sharpen, .. }`. Verify with `cargo test -p gui enhance::operation`.
- [x] 1.4 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify all three pass clean.

## 2. Frontend model

- [x] 2.1 Add the `sharpen` member (`codename`, `precision`, `strength` in unit space, documented as the denoise member is) to `Operation` in `ipc/enhance.ts`; add `sharpen` to `RUNNABLE` between `color_balance` and `upscale` and update its doc's family count and order note (design D6); make `newOperation`'s denoise case fall through for `sharpen` and widen `DEFAULT_STRENGTH`'s doc to name both families (design D5). Verify with `pnpm tsc --noEmit` passing, which also proves `Unoffered` is satisfied.
- [x] 2.2 Give `test/support.tsx`'s three sharpen variants the `STRENGTH` parameter and name sharpen in the fixture's doc (design D8); extend `lib/enhancements.test.ts`: `RUNNABLE_ENHANCEMENTS` is denoise, face recovery, light adjustment, colour balance, sharpen, upscale in that order; `newOperation("sharpen", ..)` carries Moscow and a strength of 1. Verify with `pnpm vitest run lib/enhancements`.

## 3. Frontend controls

- [x] 3.1 Build `features/enhancements/SharpenOptions.tsx` as a wrapper over `IntensityOptions` (parameter `strength`, label `enhancements.strength`, mark 100, field `strength`; design D3, D4), and wire it and the `Moscow, 100%, HD` info line (a `sharpen` fall-through onto the `denoise` case, design D7) into `EnhancementRow`'s two switches, updating the info case's comment. Verify with `pnpm tsc --noEmit` and by the tests in 3.2 and 3.3.
- [x] 3.2 Add `SharpenOptions.test.tsx` covering the spec's scenarios: the control is labelled Strength; typing 150 writes 1.5; 400 clamps to the published maximum; -20 writes 0; a slider commit writes once; choosing Novgorod keeps the strength; opening the panel writes nothing. Verify with `pnpm vitest run features/enhancements/SharpenOptions`.
- [x] 3.3 Extend `AddEnhancement.test.tsx` and `EnhancementList.test.tsx`: the menu offers Sharpen after Color Balance and before Upscale; adding it leaves its panel closed; its row reads `Moscow, 100%, HD`, and `250%` for a strength of 2.5; a sharpen added after an upscale is listed between a colour balance and that upscale. Verify with `pnpm vitest run features/enhancements`.
- [x] 3.4 Run biome scoped to the files this change touched (not `pnpm check` over the repo), then the whole frontend suite, and verify both pass.

## 4. Verification

- [x] 4.1 Hand a launch check to the user rather than driving the GUI: open `cmd/perf/test.dat`, add Sharpen, confirm the `Moscow, 100%, HD` row, the panel matching Denoise's (Strength label, 0..300 slider, `100` under the thumb at 100%), a strength of 0 giving back the photograph and 200 sharpening harder than 100, and each Moscow/Petersburg/Novgorod switch re-running once. Record the outcome here.
  - Outcome (2026-09-24): run by the user on macOS; passed.
- [x] 4.2 Run `openspec validate add-gui-sharpen --strict` and verify it passes.
