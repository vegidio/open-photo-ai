## Context

See proposal.md, "Why". The state this design builds on:

- `crates/cli/src/args.rs` parses with `clap` derive. `--up` and `--fr` are `Option<ValueEnum>` arguments with
  `num_args = 0..=1`, `default_missing_value` and `ignore_case`. `--scale` uses a custom `value_parser` that names
  the range on refusal, plus `requires = "upscale"`. The "at least one enhancement" check is a hand-written clap error
  after parsing, not an `ArgGroup`, so the message can say that an enhancement is required.
- `crates/cli/src/chain.rs` maps `Args` to a `Vec<Operation>` through the `opai` constructors, with `hd`/`sd` mapped
  to `FloatPrecision`.
- `opai` already exports everything needed:
  - `Denoise::{stockholm, gothenburg, malmo}(FloatPrecision, Strength)`
  - `Sharpen::{moscow, stpetersburg, novgorod}(FloatPrecision, Strength)`
  - `Colorization::{delhi, mumbai, jaipur}(FloatPrecision)`
  - `LightAdjustment::{paris, lyon}(FloatPrecision, Bias)`
  - `ColorBalance::{rio, saopaulo}(FloatPrecision, Bias)`
  - `Strength` (0.0-3.0) and `Bias` (-1.0-1.0), each quantized to 0.001
  - `catalogue()`, which publishes each family's variants in chooser order and each parameter's range and default
    (strength 1.0, bias 0.5)
  - `Family::APPLY_ORDER`, which `Opai::process` sorts every chain into
- `crates/cli/src/look.rs` already labels all seven families on the progress bar, and its label width already fits
  the longest of them. Nothing in progress or output rendering changes.
- The library's codenames are `petersburg` and `saopaulo`. The constructors are named `stpetersburg` and `saopaulo`.

## Goals / Non-Goals

**Goals:**

- The modified `cli-enhance` requirements, with every new parse and mapping rule covered by a unit test that needs no
  model.
- No default, range or model list restated in the CLI that could drift from the library without a failing test.

**Non-Goals:**

- Any change to `opai`, `gui` or `perf`.
- A fidelity flag for Athens, Autopilot, a per-face selection, or fractional parameter values.

## Slices

The `add-cli-filters` series has one slice.

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-cli-filters-1-all` | The five enhancement flags, their models, and the four parameter flags | `add-cli-2-enhance` (archived) | **current** |

## Decisions

### D1. One `ValueEnum` per family, spelled as the library's codenames

Each family gets its own enum, like `UpscaleModel` and `FaceModel`: `DenoiseModel`, `SharpenModel`,
`ColorizationModel`, `LightModel` and `BalanceModel`. Each flag uses the same pattern as `--up`: `num_args = 0..=1`,
`default_missing_value` set to the family's first catalogue variant, and `ignore_case = true`.

The CLI names are the library's codenames. clap's derive turns `SaoPaulo` into `sao-paulo` by default, so that
variant needs an explicit `#[value(name = "saopaulo")]`. `Petersburg` is already right as it is.

A test walks `opai::catalogue()` for each of the five families and checks two things. First, that the enum's values,
in order, are the catalogue's codenames. Second, that the `default_missing_value` is the first one. Adding or renaming
a variant in the library then fails the CLI's tests, instead of the CLI silently going out of step.

*Alternative considered:* parsing the model name at runtime against the catalogue with `VariantEntry::codename` and
`from_codename`. It removes the enums, but it also loses clap's generated list of accepted values in `--help` and in
the "invalid value" error, which the existing flags rely on.

### D2. Parameters are `Option<i16>` / `Option<u16>`, defaulted in `chain.rs` from the catalogue

`--dn-str` and `--sh-str` are `Option<u16>`. `--la-bias` and `--cb-bias` are `Option<i16>`. Each has a custom
`value_parser`, the way `--scale` does. The parser refuses non-integers and out-of-range values with one message
that names the range ("the strength must be a whole number from 1 to 300"). It also sets `requires` on the matching
enhancement flag. The bias arguments set `allow_negative_numbers = true`, so `--la-bias -20` is read as a value and not
as an unknown short flag.

They are `Option` and not clap defaults. A default would make clap treat the argument as always present, and I'm not
certain how that interacts with `requires`. The `Option` sidesteps the question. `chain.rs` resolves `None` to the
default the catalogue publishes and divides the percentage by 100 to build the `Strength` or `Bias`. The help text
still says "default 100" or "default 50", and a test checks those numbers against `catalogue()`, so the help can't
drift either.

Every whole percentage in range divided by 100 is exactly representable after the library's 0.001 quantization. The
constructor therefore never refuses a value the parser accepted, and the `expect` on it is an invariant, not error
handling.

*Alternative considered:* one shared `--strength` and `--bias`. Rejected during exploration, because
`--dn --sh --strength 200` is ambiguous and a run can't denoise and sharpen at different strengths.

### D3. `plan` pushes operations in `Family::APPLY_ORDER`

`plan` already relies on the library to sort the chain. It still pushes in the library's order (denoise, face
recovery, colorization, light adjustment, colour balance, sharpen, upscale), so `Plan` compares equal whatever the flag
order. The existing `the_flags_order_does_not_change_the_chain` test is extended to the new families.

### D4. Messages and help

The `parse_from` check covers all seven `Option`s. Its message lists each flag with its alias, in apply order. The
`Args` doc comment, which clap shows as the long help, replaces "Face recovery runs before an upscale" with the full
apply order.

## Crates and platforms

- **Crates:** only `cli` changes. All the logic is the mapping from terminal vocabulary to library constructors. The
  ranges, defaults, quantization and apply order stay in `opai`, which this design reads and does not copy.
- **rust-sak:** no feature used or needed. Argument parsing is not infrastructure.
- **New crates:** none. `clap` is already a CLI dependency.
- **Model sessions, budgets and providers:** untouched. The CLI hands `Opai::process` a longer chain, and session
  ownership and lifetime stay as `add-cli-2-enhance` describes.
- **Validation:** the parse and plan tests are platform-independent. An end-to-end run of each new family is planned
  on macOS (CPU and CoreML). Windows and Linux, and CUDA and TensorRT, have not been tested by this change. They run
  through the same library path the GUI already uses for these families.

## Risks / Trade-offs

- [`allow_negative_numbers` on an argument whose value is optional] → it isn't. The bias arguments always take
  exactly one value, so the setting can't swallow the next flag. A test runs `--la --la-bias -20 -o out.png`.
- [A user writes `--la -20`, expecting the bias on the flag itself] → clap tries to read `-20` as the model name and
  refuses it, listing `paris` and `lyon`. The error is clear enough, so there's no special case for it.
- [The strength floor of 1 differs from the GUI slider's 0] → deliberate. 0 applies nothing, so on the command line
  it's a mistake rather than a choice. The spec states the range.
- [Colorizing a colour photograph] → the CLI doesn't check this, just as the GUI doesn't. The library's behaviour is
  what the user gets.
