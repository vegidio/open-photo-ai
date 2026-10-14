## Context

See proposal.md - Why. These facts were checked in the code and shape the design:

- **The vocabulary is in `opai` already.** `opai::ExecutionProvider` has `Auto`, `Cpu`, `CoreMl`, `Cuda`, `TensorRt`
  and `WebGpu`. `ALL` lists them, `as_str` gives their user-facing spelling, and `FromStr` parses them without regard
  to case.
- **So is the machine report.** `Opai::providers()` returns `SupportedProviders`. Its `supports(p)` is `true` for
  `Auto` and `Cpu` on every machine, and for an accelerator only where the install plan (CUDA, TensorRT), the OS
  version (CoreML) or the loaded runtime (WebGPU) offers it. `available()` lists what can be built on, without `Auto`.
  The GUI's settings pane builds its processor list from this report.
- **The library does not refuse an unsupported request.** `providers::options::resolve_chain` turns a request this
  machine cannot serve into a CPU run and keeps the `requested` value for the log (`ProviderVerdict::Downgraded`).
  That suits the GUI, whose list never offers such a request. On a command line the user can type anything, so a silent
  CPU run would hide the mistake. The refusal therefore belongs in the CLI.
- **The CLI asks for `Auto` in two places,** both by `..Default::default()`: `ExecuteOptions` for `find_chain_faces`
  and `ProcessOptions` for `process` (`crates/cli/src/run.rs`). A comment in `provenance` says `AsRequested` and
  `Downgraded` "cannot happen under `Auto`". That stops being true here.
- **Short flags in use:** `-q`, `-r` and `-o`. `-p` is free.

## Slices

The CLI progress redesign is delivered as three changes. **This change is change 2.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-progress-1-spinner` | In `rust-sak`: a `tui::Spinner` (braille dot, 100 ms per frame, clockwise by default with a `Direction` to reverse it), README row, example, the `26.10.6` tag moved to the new commit. Here: `Cargo.lock` follows the moved tag; the pins stay at `26.10.6` | - | **DONE** (archived 2026-10-14) |
| 2 | `cli-progress-2-processor` | In `cli`: `--processor/-p` taking `auto` (default), `cpu`, `coreml`, `cuda`, `tensorrt`, `webgpu`. Values the OS can never offer are left out of the parser and `--help` at compile time (`coreml` only on macOS, `cuda` and `tensorrt` never on macOS). After initialization the choice is checked against the machine's supported providers, the same check the GUI uses, and an unsupported one is refused with a `🧨` line naming those that are available (exit 2). The choice is handed to face detection and processing instead of the fixed `Auto`. `cli-enhance` spec delta and `CLI_REFERENCE.md` | - | **CURRENT** |
| 3 | `cli-progress-3-output` | In `cli`: `💾 Downloading dependency <name>... <spinner> <n>% Complete` (name and percentage white and bold, spinner magenta `#c792e9`, a green `✓` in its place once done); `🧠 Running inference using the processor <Name>; quality <HD/SD>` (name green, quality yellow) under it, with the blank line always under that; the bar labelled `Enhancing`, `[n/N]` counting finished images (failures included), filling smoothly as `(done + current image's chain fraction) / N`; rows `🖼️ [ 42%] Applying <Family> in the image <path>`, `Downloading the model for <Family>`, held at 99% until written, `[100%] All enhancements applied` (also for an empty chain), `🧨 [ 42%] Failed to enhance` / `Stopped enhancing` in red, with family words and `enhancements` in magenta; the same rows printed one per image when stderr is not a terminal; `cli-enhance` spec delta and `CLI_REFERENCE.md` | 1, 2 | PENDING |

Changes 1 and 2 are independent. Change 1 is already done.

## Goals / Non-Goals

**Goals:**

- One place in `cli` that knows which processor names exist in a build for a given OS. The parser, `--help` and the
  mapping to `opai::ExecutionProvider` all read from it.
- The refusal of an unsupported processor reuses the library's report and adds no rule of its own about hardware.

**Non-Goals:**

- Narrowing what initialization installs to the chosen processor. The library has no such option, and the install is
  shared with the GUI.
- Making the run cache provider-aware. The library excludes the provider from the key on purpose.
- Printing the processor on the terminal. That is change 3's `🧠` line.
- Changing `opai`. Everything this needs is public already.

## How the work divides

**All in `cli`.** The vocabulary, the machine report and the CPU fallback are `opai`'s, and the GUI uses them already.
What is added is the terminal's own: a clap value list for each OS, the spelling of the flag, and the wording and
exit status of a refusal. Nothing that the GUI would need is put into the CLI. The GUI never refuses, because its list
only offers what `available()` returns.

## `rust-sak` coverage

| Need | Covered by | Where |
|---|---|---|
| Parsing a flag's values | clap (already a `cli` dependency) | `cli` |
| What this machine offers | `opai::SupportedProviders` | `opai` (exists) |
| Anything infrastructure-like | nothing needed | - |

No `rust-sak` feature is used or needed. No crate is introduced.

## Decisions

### A `cli`-side `Processor` enum with `#[cfg]`'d arms, mapped to `opai::ExecutionProvider`

```rust
/// The execution providers, as the command line names them. Only the ones this OS can ever offer exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Processor {
    #[default]
    Auto,
    Cpu,
    #[cfg(target_os = "macos")]
    #[value(name = "coreml")]
    CoreMl,
    #[cfg(not(target_os = "macos"))]
    Cuda,
    #[cfg(not(target_os = "macos"))]
    #[value(name = "tensorrt")]
    TensorRt,
    #[value(name = "webgpu")]
    WebGpu,
}

impl From<Processor> for opai::ExecutionProvider { /* one arm each, cfg'd the same way */ }

// in Args:
/// The hardware to run on: auto (the best this machine has; the default), cpu, …
#[arg(long, short = 'p', value_enum, default_value_t = Processor::Auto, ignore_case = true)]
pub processor: Processor,
```

`#[cfg]` is resolved before `derive(ValueEnum)` expands, so a variant that does not exist on this OS is missing from
clap's value list, its `[possible values: …]` and `--help` with no extra code. This is the same derive pattern the
model flags and `Quality` use. The explicit `#[value(name)]`s stop clap from spelling `CoreMl` as `core-ml` and
`TensorRt` as `tensor-rt`, as `BalanceModel::SaoPaulo` already does for `saopaulo`.

**Alternative rejected: a `value_parser` over `ExecutionProvider::from_str`.** It would reuse `opai`'s parse, but it
would accept `coreml` on Windows and need a hand-built `PossibleValuesParser` for `--help`. Both would have to filter
`ExecutionProvider::ALL` by a `cfg!` table, which is the same table written in a less direct form. Its error would
also be an `InitError::UnknownProvider`, worded for a settings file.

**Alternative rejected: offer every value on every OS and let the runtime check refuse.** `-p coreml` on Linux would
then initialize, possibly download a runtime, and only then fail. A value no build for that OS can ever serve is a
usage error and belongs to the parser.

**Why `cuda`/`tensorrt` are `not(macos)` rather than `any(windows, linux)`:** it follows the product rule "never on
macOS". Any other target the workspace might build for keeps the values, and the runtime check refuses them there.

### The machine check runs right after initialization, before the batch

```text
initialize --> finish_install --> check processor --> stop_on_ctrl_c --> start_batch ...
                                     |
                                     +-- unsupported: Err(CliError::Usage(..)), and `run` returns
```

```rust
// in run():
let supported = opai.providers();
check_processor(args.processor, supported.supports(args.processor.into()), &supported.available())?;

fn check_processor(requested: Processor, supported: bool, available: &[ExecutionProvider]) -> Result<(), CliError>
```

`SupportedProviders` is `#[non_exhaustive]`, and no crate except `opai` can construct it. `perf` already works around
this (`crates/perf/src/report.rs`). So `run` asks the report the two questions, and `check_processor` is a pure
function of the answers. A unit test can then cover every case with plain values and no GPU. Whether the provider is
supported is still decided by `supports`, the library's rule, and `check_processor` does not re-derive it from the
list. `opai` is not widened for a test.

It has to come after initialization, because CUDA, TensorRT and WebGPU are only known once the install plan has run
and the runtime has loaded. It comes before `start_batch`, so the refusal is a single failure line under the finished
`💾` line, with no empty view drawn. The `Opai` handle is dropped on the way out as on any other early return, which
releases the single-instance claim.

**The refusal is `CliError::Usage`, exit 2.** Its message reads:

```text
🧨 the processor tensorrt is not available on this machine; use auto or cpu
🧨 the processor webgpu is not available on this machine; use auto, cpu or coreml
```

The names are the command line's lowercase spellings, the words a user would type next, rather than `as_str`'s
`TensorRT`. The list is `auto` followed by `available()`, mapped back to `Processor` names. `auto` is always valid,
and it is the most useful thing to suggest. An available provider that this build has no flag value for cannot occur,
because `available()` only reports what this OS offers. It is skipped rather than unwrapped all the same.

**Alternative rejected: exit 1, as an initialization failure would.** The machine is fine, and the user asked for
something it does not have. Exit 2 is what "Exit status says how the run ended" gives an argument that cannot be used
as asked, and a script can tell the two apart.

**Alternative rejected: warn and run on `auto` or the CPU.** That is the library's own softening, the one this check
exists to avoid. Someone who typed `-p cuda` is benchmarking or debugging, and a silent CPU run would mislead them.

### Both library calls get the same provider

`run` converts `args.processor` once to an `ExecutionProvider` and threads it into `enhance`. `enhance` sets
`provider` on both `ExecuteOptions` (face detection) and `ProcessOptions` (the chain), so the faces are found on the
same hardware the chain runs on. The `provenance` comment that said `AsRequested`/`Downgraded` cannot happen is
rewritten, because with `-p` both can now happen. Its match arms and wording already handle them.

### Ownership and lifetime of the provider choice

The choice is a `Copy` value parsed once per process. It is checked once, against the `SupportedProviders` that the
`Opai` handle fixed during initialization, and it is passed by value into every `ExecuteOptions`/`ProcessOptions` of
the run. Sessions stay owned by the handle's `SessionCache`, which builds and keeps them per artifact for the
requested/resolved pair. One CLI process asks for one provider, so each model gets one ladder, and a model that
declines its accelerator falls to the CPU for the rest of the run, as under `Auto` today. Nothing outlives the
handle. The handle drops at the end of `run` as before, which releases sessions, the claim and the cache writes.

## Risks / Trade-offs

- **[`-p cpu` on an NVIDIA machine still downloads CUDA and TensorRT the first time.]** → Documented in
  `CLI_REFERENCE.md`. Narrowing the install would split it from the GUI's and needs a library option. If it is ever
  wanted, it is a separate change in `opai`.
- **[Comparing processors with the same chain within 24 hours shows the cache, not the hardware.]** → Documented: the
  log says `served from cache`. A provider-aware cache is a library decision that this change does not make.
- **[A supported provider can still fail to build a session, and the run then goes on on the CPU without saying so
  on the terminal.]** → The log records `ran on CPU (TensorRT could not open every model)`. Change 3's `🧠` line names
  the processor asked for. Making a fallback visible on the terminal is out of scope.
- **[The `cfg` table duplicates knowledge `opai` has at runtime.]** → It holds only which values can exist on an OS,
  which is fixed at compile time. Whether this machine offers them is still decided by `opai` alone. A sixth provider
  added to `ExecutionProvider` makes the `From` impl's match non-exhaustive, so it will not compile until the CLI
  adds an arm.
- **[Platforms.]** → The parser and check are covered by unit tests on all three CI platforms. Each platform's test
  asserts its own value list. The manual checks run on macOS only: `auto`, `cpu`, `coreml` and `webgpu`. CUDA and
  TensorRT on Windows and Linux, and `-p webgpu` on Windows and Linux, are **untested** by hand in this change.

## Migration Plan

Additive. With no `-p`, the CLI asks for `Auto` exactly as before. To roll back, revert the change. No stored state,
settings file or dependency is involved.
