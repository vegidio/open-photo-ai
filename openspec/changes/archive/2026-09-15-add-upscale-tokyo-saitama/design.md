## Context

See proposal.md — Why. What matters here is the state the code is actually in.

Tokyo and Saitama are not unimplemented; they are implemented and refused. Everything below the gate
exists and is exercised by tests today:

```
                    TOKYO / SAITAMA: WHAT SHIPS TODAY
  +--------------------------------------------------------------------+
  |  variant rows      TOKYO, SAITAMA consts, FOUR_ONLY buckets   [ok]  |
  |  enum arms         UpscaleVariant::{Tokyo,Saitama}            [ok]  |
  |  codename/label    "tokyo"/"Tokyo", "saitama"/"Saitama"       [ok]  |
  |  from_codename     both resolve at Fp16 and Fp32              [ok]  |
  |  catalogue         both listed, both precisions               [ok]  |
  |  published.json    4 artifacts, pinned size + sha256          [ok]  |
  |  resolve()         Resolution::Passes; 8x tested              [ok]  |
  |  tiling            global 256/16, no per-variant geometry     [ok]  |
  |  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -   |
  |  profile()         EpProfile::default()  <-- MISSING               |
  |  chain::plan_one   explicit refusal      <-- THE GATE              |
  +--------------------------------------------------------------------+
```

Three constraints shape everything below.

**The gate is circular.** `perftest` deliberately keeps no list of what the library runs — it finds
out by attempting one, and drops what the library declines. So `perftest tokyo` today does not
measure Tokyo; it prints `declined: its execution-provider profile has not been measured yet`. The
tool built to satisfy the gate's condition is blocked by the gate.

**The `.onnx` files are the same files.** The models are unchanged from the reference implementation
and published at the same Hugging Face repository; `published.json` pins the same artifacts. Same
weights, same node counts, same op mix. What differs between the two projects is the binding layer
(`ort` against `onnxruntime_go`) and the session plumbing, not the graph the settings were measured
against.

**The measurement hardware available to this change is an Apple Silicon Mac.** CoreML and the CPU
provider can be measured; CUDA and TensorRT cannot.

## Goals / Non-Goals

**Goals:**

- Both variants run through the same pipeline Kyoto runs through, with no new pipeline code.
- Both carry execution-provider settings that have been justified against the graph, not inherited
  silently.
- The `#[expect(dead_code)]` markers on `CpuAndGpu` and `Sequential` come off because a model uses
  them, not because they were annoying.
- What was verified on this hardware, and what was taken on the reference's word, is separable by a
  later reader.

**Non-Goals:**

- **Osaka.** Its `Resolution::Graphs` branch stays refused; diffusion is a different contract and its
  own change.
- **The seven non-upscale families.** Each lands with its own pipeline.
- **The remaining nine unmeasured profiles.** They arrive with the families that can run them, as
  `add-inference-2-providers` planned.
- **Re-deriving the reference's measurements from scratch.** This change confirms direction and
  absence-of-failure on the hardware it has; it does not re-run a CUDA sweep it has no CUDA for.
- **Any new `EpProfile` field.** Both settings these two variants need already exist as typed fields
  and are already wired end to end.

## Slices

Not sliced. Twelve tasks in one change, `opai` only. This is the tail of a decision recorded in
`add-inference-5-kyoto` (D10) rather than a member of a numbered series, and the series it descends
from — inference 1-6, then perf 1-3 — is archived and finished.

## Decisions

### D1. Lift the gate and write the profiles in one change, not two

The obvious slicing — lift the gate untuned so `perftest` can measure, then tune in a second change —
is wrong here, and Tokyo is why.

On CoreML's default compute units the Apple Neural Engine compiler cannot compile a SwinIR at all.
The reference records `_ANECompiler : ANECCompile() FAILED` once per transformer block, a session
build of **1067 seconds**, and a run **22x slower than the GPU**; `SpecializationStrategy=FastPrediction`,
which routes through the same compiler, wedged the process at **11.5 GB resident**. `CPUAndGPU` is
chosen for what it *excludes* — it is the one setting that cannot reach that compiler.

So an untuned Tokyo on a Mac is not a slow model a user might grumble about. It is an eighteen-minute
hang followed by a 22x-slow run, on the only Mac architecture this project ships inference for (no
`darwin_amd64` runtime is published, so every Mac here is Apple Silicon). A slice that shipped that
state, even briefly, would be shipping the exact failure the gate was erected to prevent.

**Alternative considered — an escape hatch in `opai` letting the benchmark bypass the gate.** Rejected.
It would be public surface whose only meaning is "this library is midway through a piece of work",
and it would outlive the work. The gate is temporary; an API describing it would not be.

**Alternative considered — measure by hand outside the library.** Rejected as a false economy: a
number measured through a different session-building path is not a number about what ships.

**What the confirming sweep actually found, recorded here because it weakens this decision's own
premise.** On ONNX Runtime 1.26 the pathology above did **not** reproduce: under the default `ALL`,
Tokyo's first session build took 48.3s rather than 1067s, and no `ANECompile` record reached either
`ort`'s forwarded C++ diagnostics or the system log. The reference's figures come from *forcing*
`CPUAndNeuralEngine`; under `ALL` this runtime's planner appears simply not to choose the Neural
Engine for this graph. So the eighteen-minute hang is not what a slice shipping an untuned Tokyo
would have produced here, and this decision's argument for not slicing is weaker than it reads above.
It is not empty — `ALL`'s safety rests on a planner decision rather than on anything declared, and a
declared `CPUAndGPU` is still worth 23% at FP16 — but the honest summary is that the two changes
could have been sliced and the cost of not slicing them was low. `UpscaleVariant::profile` carries
the same finding where a reader of the code will meet it.

### D2. Seed both profiles from the reference, then confirm — rather than deriving from zero

This is what breaks the circularity (Context), and it is what Kyoto already did: Kyoto's Rust profile
carries the reference's reasoning *and* its re-verification, including the export-regression story
that changed the answer.

It is sound here specifically because the graph is byte-identical (Context). A profile is a claim
about an op mix — Tokyo's 776 reshapes, 344 transposes, 110 normalizations against 36 convolutions;
Saitama's 96 convolutions, 75 LeakyRelus, 72 concatenations and no attention — and the op mix is a
property of the file, not of the runtime reading it.

What the confirmation step then has to establish is narrower and genuinely local: that `ort`'s session
plumbing applies the settings, and that the published artifact is the export the settings were
measured against (D5).

### D3. Tokyo declares both settings at both precisions; Saitama declares one at FP16 only

```
  tokyo    -> CoreMlComputeUnits::CpuAndGpu            both precisions
              ExecutionMode::Sequential                both precisions

  saitama  -> CoreMlComputeUnits::CpuAndNeuralEngine   FP16 arm only
              (FP32 arm stays EpProfile::default())
```

The asymmetry is the point, and Rust states it better than the reference can. The reference needs a
`func(Precision) EPProfile` for Tokyo and an `Fp16Only` wrapper for Saitama because a Go variant
cannot carry its own precision; here the precision is already inside the enum arm, so both shapes are
just patterns.

**Tokyo at both precisions** because what excludes the Neural Engine is the graph's structure — window
attention — which does not change with the export precision. On speed alone the restriction would only
pay for itself at FP16 (1.67x on an M2 Max; FP32 is a wash), but speed is not what it is for.

**Saitama at FP16 only**, and the FP32 arm is load-bearing rather than a formality — the same reasoning
`UpscaleVariant::profile` already documents for Kyoto. CoreML's typed execution bars an FP32 MLProgram
from the Neural Engine outright, so asking for it there does not get it; it drops the graph onto the
CPU. For Kyoto that is a 9x regression at 2x and 20x at 4x. Writing one value for both precisions
would be a measured regression wearing the shape of a harmless generalisation.

Saitama's profile is Kyoto's, reached independently from the same RRDBNet op mix. That it converges is
worth stating in the code comment rather than deduplicating: two variants agreeing because their
graphs agree is a fact about the graphs, and a shared constant would hide it.

### D4. `Sequential` is adopted on the reference's CUDA figures, and confirmed here by a different experiment

This is the one setting this change cannot measure in the direction that motivates it.

```
 +-------------------------------------------------------------------+
 | coreml_compute_units = CpuAndGpu                 MEASURABLE HERE   |
 |   Default ALL -> ANECCompile() FAILED per block, 1067s build,      |
 |   22x slow. Observable directly: ort's `tracing` feature forwards  |
 |   the runtime's C++ diagnostics into opai.log, so the compiler     |
 |   failures are records rather than a silent stall.                 |
 +-------------------------------------------------------------------+
 | execution_mode = Sequential                   NOT MEASURABLE HERE  |
 |   Justified by CUDA: -26.9% FP32, -18.1% FP16.                     |
 |   On a Mac: FP32 nothing, FP16 -3.5% -> under this project's own   |
 |   ~3% noise floor. CoreML takes all 2682 nodes as one partition,   |
 |   so the question cannot even arise there.                         |
 +-------------------------------------------------------------------+
```

**It is adopted anyway.** Three reasons, in order of weight:

1. The mechanism is a property of the file, not the provider. 2682 nodes arranged as 54
   window-attention blocks in a chain give the inter-op pool no two independent nodes to hand out, so
   all it can do is charge a thread handoff per node — charged per `Run`, not once. That argument
   reads off the graph, and the graph is the one that ships.
2. Output is bit-identical in both modes. This is a setting, not a trade, which is why the spec delta
   states it as a requirement rather than leaving it as a measurement footnote.
3. The downside is bounded at zero on the hardware that cannot check it. On CoreML the provider takes
   the whole graph as one partition; on the Mac the setting measures as a tie at FP32 and a small win
   at FP16. Withholding it would make every CUDA user pay ~27% for a number nobody on this project can
   currently disprove.

**What is confirmed here instead.** The reference reports Tokyo as *a small win* from sequential on
the CPU provider, where Kyoto is *a small consistent loss* — RRDB's dense blocks feed each
concatenation from several branches at once, which is the wide independent structure the inter-op pool
exists for. That **sign flip between the two graphs** is measurable on this hardware with
`perftest --provider cpu`, and it tests the actual claim: that the graph's shape is what decides the
answer. A confirmed sign flip turns an inherited number into a locally verified mechanism, which is a
materially stronger position than "the reference said so" even though it is weaker than a CUDA sweep.

**What the substitute experiment found, recorded here as D1 records its own.** The sign flip
reproduced, and by a wider margin than the reference's "small win" led this decision to expect. At
FP32 on the CPU provider, 640x640 in, median run:

```
                                              parallel   sequential
    tokyo   (chained window attention)         133.5s      101.2s     -24.2%
    kyoto   (dense RRDB blocks)                 62.5s       64.6s      +3.3%
```

Two graphs, one setting, opposite answers. Tokyo's -24.2% on a provider that was never the argument
lands within a couple of points of the -26.9% CUDA figure that was, which is more than this decision
claimed it would get: the box above quotes the reference's Mac figures of "FP32 nothing, FP16 -3.5%",
but those are CoreML's, where the provider takes the whole graph as one partition and the question
cannot arise. On the provider that does have an inter-op pool the mechanism is plainly visible.
Bit-identical output was confirmed separately, on both CoreML and the CPU provider, by the live
checks in `inference/live.rs` — reason (2) above was inherited too, and is now checked.

**Alternative considered — omit `Sequential` and leave it to whoever owns a CUDA box.** Rejected on
(3): it costs real users a measured 27% to avoid an unfalsifiable risk, and it would leave Tokyo's
profile half-written in a way the next reader would have to reconstruct.

### D5. Saitama's FP16 export is verified before the profile is written, not after

The sharpest risk in this change, and it is cheap to retire.

`saitama.go` records that `CPUAndNeuralEngine` was worth **nothing** until the FP16 export was fixed,
and that against the export which shipped before the fix the Neural Engine was *the worst* of the
three choices rather than the best:

```
  saitama FP16, same profile, two different exports:

     pre-fix export  ->  Neural Engine 50% BEHIND doing nothing
     re-export       ->  Neural Engine best, by -54%
     GPU             ->  76ms on both (never cared)

  Partition counts are IDENTICAL on both: ORT hands CoreML all 282
  nodes as one partition either way. Only a timing tells you.
```

The cause is `onnxconverter-common` refusing to convert `Resize` to float16, which leaves the two
upsample `Resize` nodes at FP32 and four `Cast` nodes wrapped around the largest tensors in the graph.
Kyoto had the identical defect from the identical tool, and `UpscaleVariant::profile` already
generalises the lesson: on CoreML, measure an FP16 graph's compute units only after checking that the
conversion left no FP32 islands, or the measurement is of the conversion rather than of the model.

So the check comes first and it is a **graph inspection, not a timing**: does `up_saitama_4x_fp16.onnx`
(pinned at `e1fc6151…`, 8,984,791 bytes) carry FP32 islands around its `Resize` nodes? If it does, the
published artifact is the pre-fix export, `CpuAndNeuralEngine` is actively wrong, and the profile must
not be written — that outcome is an open question for the user, not a thing to work around (Open
Questions).

Tokyo gets the cheaper counterpart check: confirm the shipped artifact matches the graph the profile
describes (2682 nodes, 36 convolutions), so the profile is not attached to a different export by the
same name.

### D6. The refusal's test inverts rather than moves

`the_two_unmeasured_convolutional_variants_are_gated_by_name` currently asserts that all four
variant/precision pairs are refused and that the reason "reads as untuned rather than unsupported". It
becomes the positive: all four plan, and each carries the profile its arm declares. Keeping it at the
same seam keeps the guarantee where the gate was — nothing silently plans with `EpProfile::default()`.

`models/test_support.rs`'s `tokyo()` helper needs no change. It is only a second distinct `ArtifactId`
for the session-cache and registry tests and never reaches `plan_one`.

### D7. Settings that were measured and rejected are recorded, not silently absent

Tokyo's profile has an unusually informative set of negative results, and a later reader with a CUDA
box will otherwise re-run every one of them:

- `prefer_nhwc` **loses in both precisions**, FP16 included. Athens is also transformer-bearing and
  sets it, but NHWC earns its keep by reaching cuDNN's FP16 tensor-core kernels, and 36 convolutions
  are not enough of this graph to repay layout conversions threaded through 344 transposes.
- `fuse_conv_bias=1` is worse than a loss: garbage at FP32 (max|d| 2.9e+07) and a cuDNN
  `HEURISTIC_QUERY_FAILED` at FP16.
- `trt_fp16_enable` is worth 2.66x on the FP32 export and is **rejected on principle**: it does not
  produce a faster FP32 model, it produces the FP16 model under the FP32 model's name, and on TensorRT
  alone — so the same operation would carry a different precision depending on which provider the
  machine resolved to. The honest way to take the 2.66x is to select the FP16 model.
- TensorRT has nothing left to tune regardless: it swallows all 2682 nodes into a single engine.

These go in the profile's doc comment, where `UpscaleVariant::profile` already keeps this kind of
reasoning for Kyoto. No new field, no new mechanism.

## Ownership and lifetime

Unchanged, and worth stating precisely because the rules ask for it on anything touching sessions or
providers: **this change adds no ownership.** `EpProfile` owns no resource — it is plain data, `Clone`
and `Default`, carried by value from `Operation::profile()` into `Planned` and read once when a
session's options are built. Both variants use the existing per-artifact session path, so Tokyo at 8x
holds one session for `up_tokyo_4x_*` and runs it twice, exactly as `resolve` already deduplicates and
as the existing test asserts.

The one property worth naming: `ExecutionMode` is a **session** setting rather than a provider one, so
a model that declares it declares it on every provider it is resolved to — including a CPU fallback
after a downgrade. That is the behaviour the spec delta requires and it needs no new plumbing;
`providers::options` already threads it into `sessions::build`.

## Crates and dependencies

**`opai` only.** No `gui`, no `cli`. There is no front-end work to do and none to justify: both
variants are already in the `catalogue` a chooser is built from, so a front end gains two working
entries without a line changing. `crates/perf` is *used* and not modified — it keeps no list of what
the library runs, by design.

**`rust-sak`: nothing, and nothing in this change is infrastructure.** No download, hash, extraction,
temp file, config directory, image codec, cache or hardware probe is touched; the artifacts are
already pinned and installed through the existing path. There is no gap to fill, so no question of
extending it.

**No new third-party crate.** Both settings are typed fields that already exist in
`providers::profile` and are already applied.

## Platforms and providers validated

| Platform / provider | Status in this change |
|---|---|
| macOS, Apple Silicon, **CoreML** | **Measured here.** Tokyo's compute-unit exclusion (absence of ANE compiler failures in `opai.log`, session build time) and Saitama's FP16 Neural Engine win |
| macOS, Apple Silicon, **CPU provider** | **Measured here.** The Tokyo-vs-Kyoto sequential sign flip (D4) |
| **CUDA** | **Untested here.** Both of Tokyo's settings inherited from the reference; `Sequential`'s -26.9%/-18.1% not reproduced |
| **TensorRT** | **Untested here.** Expected to be a tie on Tokyo — it fuses all 2682 nodes into one engine — which is itself unverified |
| Windows, Linux | **Untested here.** Neither declared setting is platform-conditional: the CoreML one is inert off macOS, and the session-level one is provider-independent |

Margins quoted in the profile comments are an M2 Max's. The direction follows from the op mix, which
is the same everywhere; the numbers should be re-measured before being quoted on other hardware. This
is the same caveat Kyoto's profile already carries.

## Risks / Trade-offs

**Saitama's published FP16 artifact turns out to be the pre-fix export** → `CpuAndNeuralEngine` would
be the worst of three choices, not the best, and would ship a 50% regression under the name of an
optimisation. Mitigated by making the graph inspection a task that *precedes* writing the profile
(D5), and by the fact that the check is a structural one — FP32 islands around `Resize` — rather than
a timing that could be misread. If it fails, the change stops and asks (Open Questions).

**Tokyo's `Sequential` is adopted without local confirmation on the provider that motivates it** →
Accepted deliberately, with the reasoning and the substitute experiment in D4. Bounded because output
is bit-identical and the Mac-side measurement is a tie-to-small-win, never a loss. The risk is that the
setting is merely neutral on some CUDA generation, not that it is harmful.

**The reference's CoreML numbers were taken on one machine** → The ANE compiler failure is a
compile-time capability rather than a performance margin, so it does not vary the way a timing does:
either the compiler accepts the graph or it does not. The margins do vary, and are labelled as one
machine's.

**A later reader cannot tell inherited reasoning from verified reasoning** → Mitigated by the
platforms table above and by attributing each figure in the profile comments to where it was measured.
This is the risk that the whole "state what is untested" rule exists for.

**Lifting one gate invites lifting the rest** → The remaining refusals are not the same kind of thing
and the code should keep saying so: Osaka is a contract difference, the seven families are missing
pipelines. Only the unmeasured-profile branch goes; the others keep their own reasons, and `plan_one`'s
exhaustive matches still force a new family or variant to answer.

## Open Questions

**Only one, and it is a stop-and-ask rather than a defer:** if the FP32-island inspection (D5) shows
that the published `up_saitama_4x_fp16.onnx` is the pre-fix export, this change cannot write Saitama's
profile as designed. The options at that point — ship Saitama with a default profile, ship Tokyo alone,
or re-export and republish the artifact — differ in scope and in which repository they touch, so the
answer is the user's. It is recorded here rather than guessed because it would change the specs, the
tasks and the outcome.

Everything else is settled: no question about the approach, the profile values, the slicing or the
task breakdown remains open.
