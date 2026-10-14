## Why

The CLI always runs on `Auto`. A user cannot force the CPU to rule out a GPU problem, pick CUDA over TensorRT to skip
an engine build, or try WebGPU, although the GUI has offered that choice since its settings pane. Change 3 of this
series also prints `🧠 Running inference using the processor <Name>`, so the processor has to be something the user
chose first. This change adds the flag.

## What Changes

Task estimate: **8**. The whole CLI progress redesign needs more than 15 tasks, so it is split into three changes
(see design.md - Slices). This proposal covers change 2.

**In `crates/cli`:**

- **A `--processor` flag, short `-p`,** taking `auto` (the default), `cpu`, `coreml`, `cuda`, `tensorrt` or `webgpu`,
  matched without regard to case.
- **Whether a processor can be used is decided by this computer, as in the GUI.** After initialization, the choice is
  checked with `opai::SupportedProviders::supports`, the same report the GUI builds its processor list from. That
  report offers CUDA only where a supported NVIDIA driver and GPU were found *and* the CUDA libraries were installed,
  TensorRT only where CUDA is offered *and* the GPU can run TensorRT *and* its libraries were installed, CoreML only on
  macOS 12 or newer, and WebGPU only where the runtime's plugin found a device. A processor this computer does not
  offer is refused with one `🧨` line naming the processor asked for and the ones that are available, and the command
  exits with the usage status `2`. No image is loaded and nothing is written.
- **Before that, the parser drops only what this OS can never offer.** This is not the support check. It only removes
  values no machine running that OS could ever pass: `coreml` exists only in macOS builds, and `cuda` and `tensorrt`
  do not exist in macOS builds. Such a value is refused by the argument parser like any unknown value, before anything
  is installed, and is not listed in `--help` or in `[possible values: …]`. On Windows and Linux, `-p cuda` passes the
  parser and is then accepted or refused by the check above, depending on the computer.
- **The choice reaches the library.** Face detection (`find_chain_faces`) and the chain (`process`) are asked for the
  chosen provider instead of the fixed `Auto`. A provider that passes the check but cannot build a session still falls
  back to the CPU, as the library already does. The log line records that fallback, as it does today.
- **Docs:** `docs/CLI_REFERENCE.md` gets a `-p, --processor` section, a rewritten "Hardware acceleration" section, and
  a new row in "Common errors".

**Not in this change:** the `🧠 Running inference using the processor …` line, which arrives in change 3. In this change
the processor is visible only in the log, as it is today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-enhance`: the command takes `--processor/-p`. Its values depend on the OS the command was built for. An
  unsupported choice is refused after initialization. "The command SHALL run on the best execution provider this
  machine supports" applies only to `auto` from now on.

## Impact

**Crates touched:** `cli` only (`args.rs`, `run.rs`, `error.rs`, `main.rs` module docs) and `docs/CLI_REFERENCE.md`.
The logic stays in the front end because there is none to share. The vocabulary (`ExecutionProvider`, its `FromStr`
and `as_str`), the machine report (`Opai::providers`, `SupportedProviders::supports` and `available`) and the CPU
fallback are all in `opai` already, and the GUI uses them. This change adds only what is the terminal's own: the
spelling a flag accepts, which values exist in a build for a given OS, and the wording of the refusal.

**`opai`:** unchanged. `ExecuteOptions::provider` and `ProcessOptions::provider` already take the choice.

**`rust-sak`:** nothing needed. This is argument parsing (clap) and a call to an existing `opai` API. No
infrastructure is involved.

**Third-party crates introduced:** none.

**Behaviour that stays as it is, and is documented rather than changed:**

- Initialization still installs what the machine needs, whatever `-p` says. `-p cpu` on an NVIDIA machine still
  downloads CUDA and TensorRT the first time, which keeps the install shared with the GUI and the next `auto` run. The
  library has no option to narrow the install.
- The run cache key does not include the provider (by design in `opai`). Running the same chain again with another
  `-p` within 24 hours is served from the cache and runs on nothing.
