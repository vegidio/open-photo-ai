## Context

See proposal.md - Why. These facts were checked in the code and shape the design:

- **The analysis already exists in `opai`.** `Opai::suggest(&Picture, Option<&[Family]>, Option<ExecuteOptions>)`
  returns `Suggestions { suggested, incomplete }`. A `Suggestion` names a family and nothing else, except
  `Upscale { scale }`. A `Cancelled`/`Shutdown` error means nothing comes back. Any other unreadable signal (today only
  the face detector) comes back in `incomplete`, beside the suggestions that were read.
- **Model choice is left to the front end on purpose.** `Suggestion` carries no variant or precision because that is
  a user preference and `opai` has no settings store (`autopilot/mod.rs`, the comment on `Suggestion`). The GUI
  answers it in TypeScript (`suggestedOperation` in `lib/enhancements.ts`), from `settings.models[family]`, falling
  back to the catalogue's first variant.
- **The CLI's defaults are the catalogue's first variants**, which `args::tests::the_new_families_match_the_librarys_catalogue`
  already checks. "The default model of each family" therefore means the same thing in both front ends.
- **The CLI plans before it loads.** `run` calls `chain::plan(&args, source.width, source.height)` with the header's
  dimensions, then `enhance_one` loads the `Picture`. `plan` reads the seven `Option<Model>` fields and four parameter
  fields straight off `Args`, and maps each model onto its `opai` constructor.
- **The analysis warms the detection the chain needs.** `autopilot::face` and `Opai::find_chain_faces` both run
  `Detection::for_face_recovery()` through `execute`, so the second is a run-cache hit. `Handover::detection` does not
  report a detection served from the store, so the row will not see the face-recovery slot run twice.
- **Cancellation maps already.** `CliError::from(InferenceError::Cancelled)` is `CliError::Cancelled`, exit 130.
- **Row wording is padded** to `WORDS_WIDTH` (`Downloading Face Recovery for`, 29 columns) in `look.rs`.
  `Choosing enhancements for` is 25, so it fits without widening anything.

## Goals / Non-Goals

**Goals:**

- One mapping from a model choice to an `opai` constructor, used by both manual flags and autopilot.
- The environment is read once, at startup, through a seam tests can substitute, never through `std::env` inside a
  function under test.
- No change to any run without `--ap`.

**Non-Goals:**

- Per-family parameter overrides (strength, bias, scale) through the environment. Autopilot uses the catalogue's
  defaults, as the GUI does.
- Printing the chosen families on the image's end line. The log records them (the analysis's own record), and the
  live row shows each family while it runs.
- Moving the suggestion-to-operation mapping into `opai`. That waits for a settings store there (see D1).
- Relaying the analysis's own progress (for example, the face model's first download) to the row. See D6.

## How the work divides

**All in `cli`.** `opai` is unchanged: `suggest` already takes the family filter (`off`), the provider (`-p`), and
the cancellation (Ctrl-C). The GUI is unchanged and must never read `OPAI_AP_*`. Only the new `cli` module reads
them, so the GUI cannot be affected by construction.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Reading environment variables | `std::env::var_os`, no `rust-sak` module exists for it, and none is warranted | - |
| Terminal rows | `Live`, `LiveLine` as today | `tui` |

Nothing is missing, so `rust-sak` is not changed. No third-party crate is introduced.

## Decisions

### D1. The model choice stays in `cli`

The project rule is that logic both front ends need belongs in `opai`. The analysis does, and it is there. What is
added here is *which model runs a suggested family*, and the two front ends answer that from different sources: the
GUI from its persisted settings, the CLI from its defaults and `OPAI_AP_*`, which the requirement restricts to the
CLI. Putting an environment reader in `opai` would make the GUI able to see it.

*Alternative:* an `opai` function `Suggestion -> Operation` taking a model choice. It would save the CLI a match per
family, but the GUI cannot call it (its mapping is TypeScript), so it would have one caller. It is deferred until
`opai` has a settings store, which is the point the `Suggestion` docs already name for it.

### D2. A `Selection` is what `plan` reads, filled by flags or by autopilot

`chain.rs` gains a `Selection`: the seven `Option<…Model>` fields, the four parameter percentages, and the scale.
`Selection::from_args(&Args)` reproduces today's behaviour exactly, and `plan(&Selection, Quality, width, height)`
replaces `plan(&Args, width, height)`. Autopilot builds a `Selection` per image from `Suggestions` and the
`AutopilotModels` (D4): each suggested family gets its model, no parameters (so `published_default` applies), and
`Upscale { scale }` sets `scale`, so the suggested scale is honoured as an explicit one would be.

This lands first, as a pure refactor whose existing tests stay green, so the autopilot task only adds a constructor.

*Alternative:* a second function `plan_autopilot(&Suggestions, …)` with its own seven-arm match. It would duplicate
the constructor mapping, and the two would drift the first time a model is added.

### D3. Exclusivity is clap's `conflicts_with_all`

`--autopilot` is a `bool` arg with `visible_alias = "ap"` and `conflicts_with_all` naming the twelve ids
(`denoise`, `face_recovery`, `colorization`, `light_adjustment`, `color_balance`, `sharpen`, `upscale`, `scale`,
`denoise_strength`, `sharpen_strength`, `light_bias`, `balance_bias`). clap then produces
`the argument '--autopilot' cannot be used with '--upscale [<MODEL>]'`, exit 2, before the `requires` checks on the
parameter flags run. That gives `--ap --dn-str 150` a message about the conflict, not about a missing `--denoise`.
The "at least one enhancement" check in `parse_from` adds `args.autopilot` to its list, and its message adds
`--autopilot (--ap)`.

### D4. `OPAI_AP_*` is parsed into `AutopilotModels` through an injected lookup

A new `cli/src/autopilot.rs` holds `AutopilotModels`: one `Choice<M>` per family, where `Choice` is
`Default | Model(M) | Off`. `AutopilotModels::read(lookup: impl Fn(&str) -> Option<OsString>)` reads the seven
variables. An unset or empty value is `Default`. `off` (any case) is `Off`. Anything else goes through
`M::from_str(value, ignore_case = true)`, the same `ValueEnum` the flag uses. No model is named `off`, so the word is
free.

A bad value becomes a `clap::Error` of kind `InvalidValue`, built with `Args::command().error(..)`, naming the
variable and the value and listing the family's `possible values` plus `off`. It is therefore printed and exits
exactly like a bad flag value: status 2, no `🧨` line, which "Exit status says how the run ended" already prescribes
for argument errors.

`args::parse()` calls `AutopilotModels::read(std::env::var_os)` only when `args.autopilot` is set, and only after clap
has parsed. `parse_from` stays free of the process environment, and the tests call `read` with a map. The result is
carried on `Args` as a field clap skips (`#[arg(skip)]`), so `run` needs no second source.

`AutopilotModels::families()` returns the families that are not `Off`, which is the filter handed to `suggest`. A
family switched off is therefore never analysed, as with the GUI's switch.

*Alternative:* reading the environment lazily in `run`. It would let a typo in `OPAI_AP_FACE` surface only after the
runtime download, which "Arguments and files are checked before anything is installed" forbids.

### D5. The per-image flow under `--ap`

```
 without --ap (unchanged):  plan(selection, q, w, h) --> start row --> load --> enhance --> write
 with --ap:                 start row --> load --> row: Choosing --> suggest --> plan --> enhance --> write
                                                                        |
                                                   families = AutopilotModels::families()
                                                   options  = { provider: -p, cancel, on_progress: None }
```

`enhance_one` takes a `Planner` that is either `Fixed(Plan)` or `Autopilot(&AutopilotModels)`. For `Autopilot` it
loads, sets the row to `Choosing`, calls `opai.suggest`, and then:

- on `Err`, converts through the existing `From`, exactly as an error from the chain is: `Cancelled` becomes
  `CliError::Cancelled`, which ends the batch as a stop (exit 130), and `Shutdown` stays `CliError::Inference`, which
  fails that image (exit 1)
- on `Ok` with `incomplete: Some(error)`, logs a `warn` naming the image and the error, and goes on
- builds the `Selection`, then the `Plan`, from the picture's own dimensions, logging its notes as today

The step after the analysis (the warning and the plan) is a pure function, `plan_suggested`, so the incomplete
analysis is tested with a hand-built `Suggestions` and no runtime.

An empty plan takes the existing empty-chain path, which already writes the picture unchanged and logs that no
enhancement ran.

Sessions and providers: the analysis acquires its detection session through `execute`, which holds the handle for
the length of the call and drops it before returning. Nothing outlives `suggest`, so no new ownership or lifetime
exists. `-p` is passed to the analysis as it is to the detection and the chain, so all three run on the same
requested provider with the library's usual CPU fallback.

### D6. The `Choosing` row stage, at 0%, with no relayed progress

`look::Activity` gains `Choosing`, worded `Choosing enhancements for`, padded to `WORDS_WIDTH` like the others.
`View::choosing(index)` sets the current image's state to `Working(Choosing)`, in both modes, mirroring into the live
row. The percentage stays at the image's tracked fraction, which is `0` because nothing has reported yet. Once the
analysis returns, `View::chosen(index)` sets the row back to `Working(Preparing)`, so an image given an empty chain
reads `Preparing to enhance` until it is written, as the spec's "An image being analysed" scenario requires, and an
image given a chain moves on with its first report.

The analysis is passed `on_progress: None`. Its only reporting work is the face detection, and relaying it would mean
mapping a detection report onto a chain that does not exist yet. The `Handover` that splits the bar is built from the
planned chain, which is only known after the analysis. Once the chain starts, the detection is served from the
store and reports nothing, and the chain's own reports move the row as today.

*Alternative:* relaying `Installing` as `Downloading Face Recovery for` during the analysis, so a first-run detector
download is visible. Rejected for now: the row would claim a face recovery the image may not get. See Risks.

## Risks / Trade-offs

- **[First run looks stalled]** On a machine without the detector, the first image sits at
  `[  0%] Choosing enhancements for` while the model downloads. → The batch bar's `ETA` and elapsed time keep
  ticking, and the log records the install. If it proves confusing, a later change can add a `Downloading` sub-state
  for the analysis.
- **[Run cache off]** With the run cache in its `none` mode, the chain's detection is not a hit and runs a second
  time. → It is correct, only slower, and it is the same cost the GUI pays in that mode.
- **[Variable names vs family names]** `OPAI_AP_FACE`, `OPAI_AP_LIGHT` and `OPAI_AP_COLOR` do not spell the flag
  names, and `OPAI_AP_COLOR` sits next to `OPAI_AP_COLORIZATION`. → The names are the user's choice. `--help` for
  `--autopilot` and `CLI_REFERENCE.md` list all seven, and an error names the variable it rejected.
- **[A stale variable]** A variable left set from an earlier session silently changes a later `--ap` run's model. →
  It is visible in the log, which records each operation's codename. This is the cost of configuring through the
  environment, which the user chose.
- **[`docs/CLI_REFERENCE.md` has uncommitted edits]** The working tree carries a local change to it. → The docs task
  edits on top of it, and does not reset it.

## Platforms

To be validated by hand on macOS (CPU and CoreML, `-p auto` and `-p cpu`). The unit tests run on all three CI
platforms. Windows and Linux terminals, and CUDA/TensorRT, stay untested by hand. Nothing in this change is
platform-specific beyond what `-p` already handles.

## Migration Plan

None. The flag is new, the variables have no effect without it, and no stored state changes.
