## 1. The chain reads a `Selection`

- [x] 1.1 In `crates/cli/src/chain.rs`, add `Selection` (the seven `Option<…Model>` fields, the four parameter percentages, `scale: Option<Scale>`) with `Selection::from_args(&Args)`, and change `plan` to `plan(&Selection, Quality, width, height)`; update `run.rs`'s call and every test in `chain.rs` to go through `Selection::from_args` (design.md - D2); verify `cargo test -p cli chain` is green with no expected value changed

## 2. The flag

- [x] 2.1 In `crates/cli/src/args.rs`, add `pub autopilot: bool` as `--autopilot` with `visible_alias = "ap"`, `conflicts_with_all` the twelve enhancement and parameter ids, and help text listing the seven `OPAI_AP_*` variables and `off`; add `args.autopilot` to the "at least one enhancement" check and `--autopilot (--ap)` to its message (design.md - D3); add tests: `--ap` alone parses, `--ap` with each of the twelve flags is an `ArgumentConflict` naming both, and the no-enhancement message lists `--autopilot (--ap)`; verify `cargo test -p cli args` is green

## 3. The environment

- [x] 3.1 Add `crates/cli/src/autopilot.rs` with `Choice<M> { Default, Model(M), Off }`, `AutopilotModels` (one `Choice` per family), `AutopilotModels::read(lookup)` (unset/empty → `Default`, `off` in any case → `Off`, otherwise `ValueEnum::from_str(value, true)`), a bad value as an `InvalidValue` `clap::Error` naming the variable and value and listing the family's models plus `off`, `families()` (those not `Off`), and `model_for(family)` resolving `Default` to the flag's default (design.md - D4); add tests with a map lookup for every variable's default, a named model in mixed case, `off`, an empty value, and `OPAI_AP_UPSCALE=paris` refused listing `kyoto, osaka, tokyo, saitama, off`; add a test that every `Default` resolves to the catalogue's first variant; verify `cargo test -p cli autopilot` is green
- [x] 3.2 Carry the models on `Args` (`#[arg(skip)] pub autopilot_models: Option<AutopilotModels>`), filled in `args::parse()` from `std::env::var_os` only when `args.autopilot` is set, and exited through `clap::Error::exit` on a bad value; keep `parse_from` free of the process environment; add a test that `parse_from` with `--up` and no `--ap` leaves it `None`; verify `cargo test -p cli` is green and `grep -rn "OPAI_AP_" crates/gui crates/opai` finds nothing

## 4. Suggestions become a chain

- [x] 4.1 Add `Selection::from_suggestions(&[Suggestion], &AutopilotModels)` in `chain.rs`: each suggested family gets its model from `model_for`, no parameter percentages, and `Upscale { scale }` sets `scale` (design.md - D2); add tests that every family's suggestion with default models plans the bare flag's operation, that `OPAI_AP_UPSCALE=osaka` with `-q sd` plans Osaka INT8, that the suggested scale is used even above 4 MP, and that an empty slice plans an empty chain; verify `cargo test -p cli chain` is green

## 5. The row

- [x] 5.1 In `crates/cli/src/look.rs`, add `Activity::Choosing`, worded `Choosing enhancements for`, padded to `WORDS_WIDTH` (design.md - D6); extend the alignment test so every state, `Choosing` included, puts the path in the same column, and add exact-text tests for the coloured and uncoloured `[  0%] Choosing enhancements for` row; verify `cargo test -p cli look` is green
- [x] 5.2 In `crates/cli/src/progress.rs`, add `View::choosing(index)` setting the image's state to `Working(Choosing)` in both modes and mirroring it into the live row without changing the fraction, and `View::chosen(index)` setting it back to `Working(Preparing)` once the analysis returns; add a test that a report after `choosing` moves the row on to `Applying`, and one that an image ended straight from `Choosing` with an empty chain reads `[100%] Enhancements applied to`; verify `cargo test -p cli progress` is green

## 6. Wiring

- [x] 6.1 In `crates/cli/src/run.rs`, make `enhance_one` take a `Planner` (`Fixed(Plan)` or `Autopilot(&AutopilotModels)`); for `Autopilot`, after loading call `view.choosing`, then `opai.suggest(&picture, Some(&models.families()), Some(ExecuteOptions { provider, cancel, on_progress: None, .. }))`, map errors through `CliError::from` (`Cancelled` stops the run, `Shutdown` fails the image), log `incomplete` as a `warn` naming the image, then plan from `Selection::from_suggestions` with the picture's own dimensions and log its notes (design.md - D5); `run` plans per image only without `--ap`; update the module diagram; verify `cargo test -p cli run` is green and `cargo doc -p cli --no-deps` builds without warnings

## 7. Documentation

- [x] 7.1 In `docs/CLI_REFERENCE.md`, on top of the uncommitted local edits, add an "Autopilot" section (what `--ap` does, that it cannot be combined with the enhancement and parameter flags, the table of seven `OPAI_AP_*` variables with their default models, `off`, empty = unset, ignored without `--ap`, the GUI unaffected, an image that needs nothing is still written, examples including `OPAI_AP_UPSCALE=osaka opai photos --ap -o out`), add `--autopilot` to the options list, and add the `Choosing enhancements for` row to "Terminal output"; verify `cargo test -p cli every_example_in_the_reference_parses` is green (extend `documented_commands` to strip a leading `OPAI_AP_*=… ` assignment, as it already strips `NO_COLOR=1 `)

## 8. Verification

- [x] 8.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace --locked`; verify all three are green
- [x] 8.2 Verify by hand on macOS with copies of `fixtures/test.dat` (one as-is, one downscaled under 1 MP, one converted to grayscale): `opai dir --ap -o out` shows `Choosing enhancements for` and then different families per image, and writes all three; `OPAI_AP_UPSCALE=saitama` runs Saitama (log shows `up_saitama_fp32`); `OPAI_AP_COLORIZATION=off` leaves the grayscale copy uncolorized; `OPAI_AP_FACE=tokyo` exits 2 before the `💾` line; `--ap --up` exits 2 naming both flags; `-p cpu -q sd` runs FP16; a Ctrl-C during `Choosing` ends with `Stopped enhancing` and exit 130; the log records `analysis suggested enhancements` per image. Windows and Linux stay untested by hand, as design.md records
