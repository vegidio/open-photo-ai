## 1. Arguments

- [x] 1.1 In `crates/cli/src/args.rs`, add `DenoiseModel`, `SharpenModel`, `ColorizationModel`, `LightModel` and
  `BalanceModel`, with `saopaulo` spelled explicitly. Add the five `Option` flags with their aliases, default models
  and `ignore_case`, following the `--up` pattern (design D1). Verify with parse tests: bare flags give the default
  models, named models parse in any case, and an unknown model is an `InvalidValue` listing the accepted names.
- [x] 1.2 Add `--dn-str`, `--sh-str` (`Option<u16>`) and `--la-bias`, `--cb-bias` (`Option<i16>`), with value parsers
  that refuse fractions and out-of-range values naming the range, `requires` on their enhancement, and
  `allow_negative_numbers` on the bias arguments (design D2). Verify with tests: `--dn-str 0`, `301` and `150.5` are
  `ValueValidation` errors naming "1 to 300"; `--cb-bias 101` names "-100 to 100"; `--la --la-bias -20` parses to
  `-20`; `--sh --dn-str 150` is `MissingRequiredArgument` naming `--denoise`.
- [x] 1.3 Extend the "at least one enhancement" check to all seven flags, listing each with its alias, and rewrite the
  `Args` doc comment to state the full apply order (design D4). Verify with `no_enhancement_is_a_usage_error`, which
  checks that every flag appears in the message, and a test that `--cl` alone parses.
- [x] 1.4 Add a test that walks `opai::catalogue()` for the five families. It asserts that each enum's values, in
  order, equal the catalogue's codenames, that each flag's default model is the first one, and that the documented
  defaults (100 and 50) equal the published strength and bias defaults times 100. Verify by running it, and by
  temporarily reordering one enum to watch it fail.

## 2. Chain

- [x] 2.1 In `crates/cli/src/chain.rs`, map the five families to their `opai` constructors in `Family::APPLY_ORDER`.
  Resolve a missing parameter to the catalogue default and convert the percentage by dividing by 100 (design D2, D3).
  Verify by extending `every_row_of_the_quality_table` with every new model at `hd` and `sd`, and adding a test that
  `--dn --dn-str 200 --sh --sh-str 80 --la --la-bias -20 --cb` builds strengths 2.0 and 0.8 and biases -0.2 and 0.5.
- [x] 2.2 Extend `the_flags_order_does_not_change_the_chain` to the new families (`--up --sh --dn` against
  `--dn --sh --up`). Verify it passes and the chain is denoise, sharpen, upscale.

## 3. Docs and verification

- [x] 3.1 Update the README's CLI section: the usage line, the options table with the five flags and four parameters,
  and one example using a new family. Verify every flag in the table matches `opai --help`.
- [x] 3.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test -p cli` with no warnings
  or failures. Then run `opai fixtures/test.dat --dn --sh --cl --la --cb -o <scratch>/out.png` on macOS, once at the
  default provider and once at `-q sd`. Confirm both write an image and the progress bar steps through the five labels
  in apply order.
