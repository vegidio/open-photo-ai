## 1. Wire (Rust)

- [x] 1.1 Add the `Colorization { codename, precision }` arm to `Requested` in `crates/gui/src/enhance/operation.rs`, resolved through `VariantEntry::published(Family::Colorization, ..).build(precision, &ParameterValues::new())` (design D1); rewrite the module and enum docs that count six families or describe a family without an arm as a case that arrives (design D2). Verify with `cargo check -p gui`.
- [x] 1.2 Delete `a_family_this_application_cannot_run_is_refused_by_the_parse_and_names_it` (design D2), and add tests beside the sharpen ones: every published colorization codename resolves at every published precision to `Colorization::delhi`/`::mumbai`/`::jaipur` (driven from the catalogue); an unpublished codename (`moscow`) is refused as `UnknownOperation::Model { family: Colorization, .. }`. Verify with `cargo test -p gui enhance::operation`.
- [x] 1.3 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify all three pass clean.

## 2. Frontend model

- [x] 2.1 Add the `colorization` member (`codename`, `precision`, documented as carrying no value) to `Operation` in `ipc/enhance.ts`, updating the union's doc that speaks of runnable families; add `newOperation`'s `colorization` case returning the model alone (design D5). Verify with `pnpm tsc --noEmit`.
- [x] 2.2 Declare `ENHANCEMENTS` `as const satisfies readonly Enhancement[]` and move the compile check onto it (`Exclude<Operation["family"], (typeof ENHANCEMENTS)[number]["family"]>` is `never`); delete `RUNNABLE`, `Unoffered`, `_EveryRunnableFamilyIsOffered` and `RUNNABLE_ENHANCEMENTS` (design D6). Verify with `pnpm tsc --noEmit` passing, and that temporarily removing the colorization entry from `ENHANCEMENTS` makes it fail naming `colorization`. If `as const` does not type cleanly against the list's readers, take D6's fallback (a unit test) and record it here.
  - Outcome: `as const` typed cleanly against every reader, so no fallback was needed. With the colorization entry removed, `pnpm tsc --noEmit` fails with `TS2344: Type '"colorization"' does not satisfy the constraint 'never'` on the `Unoffered` check; restored, it passes.
- [x] 2.3 Update `lib/enhancements.test.ts`: drop the `RUNNABLE` and `RUNNABLE_ENHANCEMENTS` cases; `newOperation("colorization", ..)` carries Delhi and nothing else; a stored Jaipur default is honoured. Verify with `pnpm vitest run lib/enhancements`.

## 3. Frontend controls

- [x] 3.1 Make `AddEnhancement` draw `ENHANCEMENTS` directly and remove its comments about `RUNNABLE` and the screen-10 divergence (design D6). Verify with `pnpm tsc --noEmit` and by the tests in 3.4.
- [x] 3.2 Build `features/enhancements/ColorizationOptions.tsx` as `ModelTray` alone over the operation's model, writing the chosen codename and precision through `optionFor` (design D3), and wire it and the `infoModel` line (its own case, design D4) into `EnhancementRow`'s two switches. Verify with `pnpm tsc --noEmit` and by the tests in 3.3 and 3.4.
- [x] 3.3 Add `ColorizationOptions.test.tsx` covering the spec's scenarios: the panel offers Delhi, Mumbai and Jaipur at each quality and no slider, field or scale shortcut; choosing Jaipur writes Jaipur once at the chosen precision; opening the panel writes nothing. Verify with `pnpm vitest run features/enhancements/ColorizationOptions`.
- [x] 3.4 Extend `AddEnhancement.test.tsx` and `EnhancementList.test.tsx`: the menu offers all seven in pipeline order, with Colorization after Face Recovery and before Light Adjustment; adding it leaves its panel closed; its row reads `Delhi, HD`, and `Jaipur, SD` after a model change; a colorization added after a light adjustment is listed between a face recovery and that light adjustment. Update the "Six today" comment. Then run biome scoped to the files this change touched (not `pnpm check` over the repo) and the whole frontend suite, and verify both pass.

## 4. Verification

- [x] 4.1 Hand a launch check to the user rather than driving the GUI: open `cmd/perf/test.dat`, open the add menu and confirm all seven entries, add Colorization, confirm the `Delhi, HD` row, a panel holding only the model tray, a colorized result, and each Delhi/Mumbai/Jaipur switch re-running once. Record the outcome here.
  - Outcome: the user ran the launch check and it passed.
- [x] 4.2 Run `openspec validate add-gui-colorization --strict` and verify it passes.
