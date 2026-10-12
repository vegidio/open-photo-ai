## Why

The terminal command only offers upscale and face recovery. The library already ships the other five enhancement
families: denoise, sharpen, colorization, light adjustment and colour balance. A user who wants to denoise or
colour-correct a photograph from a script still has to open the GUI. This change adds those five families to the CLI.

This change is slice 1, the only slice, of the `add-cli-filters` series. See design.md, "Slices".

## What Changes

- **Five new enhancement flags.** Each takes an optional model, matched without regard to case. With no model, each
  uses the family's first catalogue variant, which is also the GUI's default:
  - `--denoise`, alias `--dn`: `stockholm` (default), `gothenburg` or `malmo`
  - `--sharpen`, alias `--sh`: `moscow` (default), `petersburg` or `novgorod`
  - `--colorization`, alias `--cl`: `delhi` (default), `mumbai` or `jaipur`
  - `--light-adjustment`, alias `--la`: `paris` (default) or `lyon`
  - `--color-balance`, alias `--cb`: `rio` (default) or `saopaulo`
  
  The model names are the library's own codenames.
- **Four new parameter flags.** Each takes a whole number, and each requires its own enhancement flag, the way
  `--scale` requires `--upscale`:
  - `--dn-str <1-300>`: the denoise strength, default 100
  - `--sh-str <1-300>`: the sharpen strength, default 100
  - `--la-bias <-100-100>`: the light adjustment bias, default 50
  - `--cb-bias <-100-100>`: the colour balance bias, default 50
  
  The values are percentages of the library's `Strength` and `Bias`, so 100 means 1.0. The defaults are the ones the
  library's catalogue publishes. Strength starts at 1 rather than the library's 0, because a strength of 0 applies
  nothing. Negative biases are written as plain numbers, as in `--la-bias -20`.
- **`--quality` covers the new families.** `hd` means FP32 and `sd` means FP16 for all five, since every one of their
  models is published at both precisions.
- **The "at least one enhancement" refusal** now lists all seven enhancement flags.
- **The help text** describes the library's full apply order, not just "face recovery runs before an upscale". Flag
  order still has no effect on the order enhancements run in.

Task estimate: about 7 tasks, within the 15-task budget.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-enhance`: the command accepts the five new enhancement flags and four parameter flags. The "at least one
  enhancement" requirement counts all seven families. The quality table covers the new families' models. The apply
  order requirement names every family.

## Impact

- **Crates touched:** only `cli`, in `args.rs` and `chain.rs` and their tests. `opai` and `gui` are not touched.
  The progress labels for all seven families already exist in `look.rs`. The README's CLI section documents the new
  flags.
- **Logic in a front end:** the CLI adds only parsing, plus the mapping from flags, model names and percentages to
  the `opai` constructors. That mapping is the CLI's own vocabulary, in the same way the GUI maps its wire request to
  an `Operation`. The ranges, defaults, quantization and apply order all come from `opai`.
- **rust-sak:** nothing needed. Argument parsing is a front-end concern, and `clap` is already a CLI dependency.
- **New dependencies:** none.
- **Not in this change:** a fidelity flag for Athens, Autopilot, and any GUI change.
