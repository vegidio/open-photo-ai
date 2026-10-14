## Why

The GUI's Autopilot analyses each photograph and adds the enhancements it calls for. The CLI has no equivalent: a
batch run applies the same flags to every image, so a folder of mixed photographs (some grainy, some small, some
black-and-white, some with faces) either gets every enhancement or needs one run per kind of photograph. The analysis
already lives in `opai` (`Opai::suggest`), so the CLI only has to call it and turn its answer into a chain.

## What Changes

Task estimate: **11**. One change, not a series.

**In `crates/cli`, observable on the command line:**

- **A new flag, `--autopilot`, alias `--ap`.** Each image is analysed after it is loaded, and the enhancements it
  calls for run on it, in the library's apply order. Different images in one run may get different enhancements.
- **`--ap` is exclusive with manual enhancement.** Giving it with any of the seven enhancement flags (`--dn`, `--fr`,
  `--cl`, `--la`, `--cb`, `--sh`, `--up`) or their parameter flags (`--scale`, `--dn-str`, `--sh-str`, `--la-bias`,
  `--cb-bias`) is a usage error (exit 2) saying the two cannot be combined. `-q`, `-p`, `-r`, `--format`, `--prefix`,
  `--suffix`, `--overwrite` and `-o` work as before. `--ap` on its own satisfies "at least one enhancement is
  required".
- **Each family runs its default model** (the same one its bare flag runs: Stockholm, Athens, Delhi, Paris, Rio,
  Moscow, Tokyo), at the precision `-q` selects, with the catalogue's default strength or bias. An upscale runs at the
  scale the analysis suggests.
- **Environment variables override the model per family:** `OPAI_AP_DENOISE`, `OPAI_AP_FACE`,
  `OPAI_AP_COLORIZATION`, `OPAI_AP_LIGHT`, `OPAI_AP_COLOR`, `OPAI_AP_SHARPEN`, `OPAI_AP_UPSCALE`. Each takes a model
  name of its family, spelled as the flag takes it and matched without regard to case, or **`off`**, which keeps that
  family out of the analysis entirely (as the GUI's per-family Autopilot switch does). An empty value is the same as
  an unset one. They are read **only by the CLI and only with `--ap`**: without `--ap` they are ignored and not
  checked, and the GUI never reads them.
- **An invalid value is a usage error** (exit 2) before anything is installed, naming the variable and listing the
  accepted models and `off`.
- **An image that calls for nothing is still written**, unchanged in content and in the requested format, exactly as
  an image whose only request was a skipped upscale is today. Its row ends `Enhancements applied to <path>`.
- **A new row stage while the image is analysed:** `🖼️ [  0%] Choosing enhancements for <path>`, padded like the
  other wordings so paths stay in one column. Once the chain starts, the row moves through the existing
  `Downloading … for` / `Applying … to` stages, which name each family as it runs.
- **An analysis that could not read a signal** (the face detector cannot be installed or opened) does not fail the
  image: the enhancements from the signals that were read still run, and the failure is logged as a warning. A Ctrl-C
  during the analysis stops the run as it does anywhere else.
- **The log records what autopilot chose for each image** (`opai`'s existing `analysis suggested enhancements`
  record). The terminal's end-of-image line is unchanged.
- **Docs:** `docs/CLI_REFERENCE.md` gains an Autopilot section (the flag, the conflicts, the environment variables,
  `off`, examples), and the new row wording.

**Unchanged:** every run without `--ap`, including its output, its errors and its exit statuses. The GUI.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-enhance`: a new requirement for `--autopilot` (the flag, its exclusivity, default models, the `OPAI_AP_*`
  overrides and `off`, the image that needs nothing, an incomplete analysis); "The command takes its inputs, its
  enhancements, and an output" lists `--autopilot` and no longer requires every image to get the same enhancements
  under it; "At least one enhancement is requested" accepts `--autopilot`; "Arguments and files are checked before
  anything is installed" covers the environment variables; "Progress is shown while the command works" gains the
  `Choosing enhancements for` row.

## Impact

**Crates touched:** `cli` only (`args.rs`, `chain.rs`, a new `autopilot.rs` for reading the environment, `look.rs`,
`progress.rs`, `run.rs`) and `docs/CLI_REFERENCE.md`.

**Why the logic is in the front end:** the analysis itself is shared and stays in `opai` (`Opai::suggest`,
unchanged). What the CLI adds is the step `opai` deliberately leaves to each front end: which model a suggested family
runs with. `Suggestion` names no model because that choice is a user preference, and `opai` holds no settings store.
The GUI answers it from its settings in TypeScript; the CLI answers it from its own defaults and from environment
variables that, by requirement, only the CLI reads. The model-to-constructor mapping the CLI already has in
`chain.rs` is reused rather than written a second time.

**`opai`:** unchanged. `Opai::suggest` already takes the family filter `off` needs, the provider `-p` names and the
cancellation Ctrl-C triggers, and its face detection warms the run cache the following `find_chain_faces` reads.

**`rust-sak`:** nothing needed. Reading environment variables is `std::env`; there is no `rust-sak` module for it and
none is called for.

**Third-party crates introduced:** none.

**Breaking:** none. `--ap` is new, and the environment variables have no effect without it.
