## Why

Tokyo and Saitama are built, published and switched off. Their variant rows, enum arms, codenames,
scale tables, catalogue entries and pinned artifacts all ship today, `Upscale::resolve` returns
`Resolution::Passes` for both, and the tiling driver already runs the 256/16 geometry their graphs
accept. The only thing standing between a user and a Tokyo upscale is one refusal in
`inference::chain::plan_one` and two `EpProfile::default()` arms in `UpscaleVariant::profile`.

That refusal was deliberate. `add-inference-5-kyoto` (D10) gated both variants because their
execution-provider settings were unmeasured, and wrote down that it was **a gate, not a limitation**:
"lifting it is one table entry per variant once someone has measured it". The series has since
delivered the thing that measures — `perftest`, in `add-perf-2-harness` and `add-perf-3-tui` — so
the condition the gate was waiting on is met, and the gate is now the only reason two working models
are refused.

It is also worth doing now rather than later because the gate's stated cost understates Tokyo's. The
refusal reads as "a user would conclude the model is slow", which is accurate for Saitama. Tokyo is a
SwinIR — 776 reshapes, 344 transposes and 110 normalizations against 36 convolutions — and on CoreML's
default compute units the Apple Neural Engine compiler cannot compile it at all: the reference
implementation records `_ANECompiler : ANECCompile() FAILED` once per transformer block, a session
that took **1067 seconds to build** and then ran **22x slower than the GPU**. Tokyo's profile is not a
speed setting, it is what stops a run looking like a hang, and that makes lifting the gate and writing
the profile one atomic change rather than two.

## What Changes

- **`UpscaleVariant::profile` gains two measured arms.** Tokyo declares `CpuAndGpu` compute units and
  `Sequential` execution mode at **both** precisions; Saitama declares `CpuAndNeuralEngine` compute
  units at **FP16 only**, leaving its FP32 arm on the defaults. Both are seeded from the reference
  implementation's measured profiles and re-confirmed here on Apple Silicon before they land.
- **The unmeasured-profile refusal in `inference::chain::plan_one` is removed.** Tokyo and Saitama
  plan, install and run like Kyoto. Osaka's `Resolution::Graphs` branch and the seven non-upscale
  families stay refused exactly as they are, each for its own unchanged reason.
- **Two `#[expect(dead_code)]` attributes are dropped** from `CoreMlComputeUnits::CpuAndGpu` and
  `ExecutionMode::Sequential`, which were written for the first model to measure them and now have
  one. Nothing else in `providers/profile.rs` changes: both fields are already wired through
  `providers::options` into `sessions::build`.
- **The gate's documentation is rewritten rather than deleted** — `lib.rs`'s crate docs and
  `Opai::process`'s "what is refused" section both name Tokyo and Saitama today, and both must end up
  describing what actually runs.
- **`the_two_unmeasured_convolutional_variants_are_gated_by_name` inverts** into a test that both
  variants plan and carry their declared profile, at both precisions.
- **A confirming `perftest` sweep on Apple Silicon** is part of the change, not a follow-up: Tokyo's
  CoreML profile is verified by the absence of the ANE compiler failures in `opai.log`, which `ort`'s
  `tracing` feature already forwards out of the runtime's C++ diagnostics.

No **BREAKING** changes. Nothing is removed from the public surface, and no caller that works today
stops working: a chain that was refused now succeeds.

## Capabilities

### New Capabilities

None. Every capability this touches already exists.

### Modified Capabilities

- `execution-providers`: the two "measured settings" requirements gain Tokyo and Saitama scenarios.
  Today they are illustrated by Kyoto alone — Kyoto at FP16 on the Neural Engine, Kyoto at FP32 on the
  defaults, and a model that declares nothing. Tokyo is the first model in this project to declare a
  compute-unit restriction that is an **exclusion** rather than a preference, and the first to declare
  a session-level execution mode; Saitama is the second model to declare a precision-specific
  compute-unit setting, which is what makes "a declaration is specific to the precision" more than a
  single-model claim.

`image-enhancement` deliberately gets **no delta**. It is the capability describing what a run accepts
and refuses, and it never named Kyoto: its refusal requirement is written generically as "where the
system has no pipeline for an operation", and its upscale requirements speak of "an upscale" and "one
set of weights" throughout. The gate was an implementation decision that the spec was already ahead
of, so lifting it makes the implementation match a requirement that does not move. Writing the gate
into the spec now, on its way out, would be recording work in progress as behaviour.

## Impact

**Crates: `opai` only.** No `gui`, no `cli`, no new crate. There is no user-visible frontend work:
both variants are already in the catalogue that a chooser is built from, so a front end offering them
needs no change and gains two working entries. **`crates/perf`'s code** is *used* by this change but
not modified — it keeps no list of what the library runs, and finds out by attempting. Its README is
the one file outside `opai` that changes, and only because it states in prose what the library runs:
its unnamed-sweep note said "within it only Kyoto", which this change makes false.

**`rust-sak`: nothing is needed from it, and nothing in this change is infrastructure.** No download,
hash, extraction, temp file, config directory, image codec, cache or hardware probe is touched. The
artifacts for both variants are already pinned in `deps/model/published.json` and installed through
the existing path. Stated explicitly because the rule requires the finding either way: the finding is
"nothing".

**Files:** `models/upscale/variant.rs` (the two profile arms and their reasoning),
`inference/chain.rs` (the refusal and its test), `providers/profile.rs` (two attributes), `lib.rs`
(two doc sections), `error.rs` (the `Unsupported` doc comment, which enumerates the refusals),
`models/operation.rs` (the library-wide "only the measured arms declare a profile" test, which names
them), `providers/options.rs` (the same enumeration in a test comment, and the option-map assertions
for the two new arms) and `inference/live.rs` (the live check that a sequential model's output is
unchanged). `models/test_support.rs`'s `tokyo()` helper is unaffected — it is only a second
distinct artifact id for the session tests and never runs inference.

**Dependencies:** none added, none changed.

**Parity:** this closes a gap rather than opening one. The reference implementation runs both
variants, and this change adopts its measured settings rather than diverging from them. The one thing
left open is verification coverage, not behaviour: the profiles will be confirmed on Apple Silicon
against CoreML and the CPU provider, and **CUDA and TensorRT stay untested here**. Tokyo's
`Sequential` setting is motivated by CUDA figures (-26.9% at FP32, -18.1% at FP16) that no machine
available to this change can reproduce; design.md states why it is adopted anyway and what is checked
on the Mac in its place.

**Estimated tasks: 12** — two verification steps on the published exports, two profile arms, the gate
and its inverted test, two documentation sections, the spec delta, two confirming sweeps, and the
`cargo fmt`/`clippy`/test pass. Comfortably inside the limit, so this is one change and not a series.
