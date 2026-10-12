## Why

Stockholm, Gothenburg and Malmö can be named, parameterised, catalogued, downloaded, opened and cached, and they cannot
run. Everything around this family already ships: `Denoise`, `DenoiseVariant`, `DenoiseParams`, `Strength`, all three
constructors, the catalogue rows, the artifact names, the run-cache tag and the serde form. The one thing between a user
and a denoised photograph is the `Self::Denoise(_)` pattern in `Operation::pipeline`'s refusal arm.

It is also the cheapest family left to run. Its contract is a tile grid at scale 1 followed by the strength blend, and
both halves already exist: `run_tiled` is what Tokyo and Saitama run through, and `inference::mix::blended` names
denoise and sharpen as its next callers in its own header. What is genuinely new is small — the per-tile divergence
guard `inference/mod.rs` deferred to this family, two ported provider profiles, and one measured here.

## What Changes

**The denoise contract: a tile grid at the photograph's own resolution, then the strength.**

- `models/denoise/process.rs`: the whole photograph run through `run_tiled` at the default 256/16 geometry, scale 1 and
  `Normalisation::Unit`, then `mix::blended` at the run's strength. The reference's `RunPipeline` and
  `BlendWithIntensity`, in that order.
- `Denoise::pipeline`, the family's contract seam, and the `Operation::pipeline` arm that reaches it. Denoise leaves the
  refusal arm; sharpen and colorization stay in it.
- The per-run `Strength` is applied **after** the tile grid and never reaches the graph, so one session serves every
  position of a slider. Above 1 it extrapolates past the model's output, as the reference does.

**The divergence guard, for Stockholm alone.** Stockholm is a NAFNet model that occasionally blows up on an
out-of-distribution tile, sending its raw output to ~1000+. Where any raw output value of a tile exceeds 3.0 in
magnitude, that tile keeps its input pixels instead of the model's output. Gothenburg and Malmö are Restormer models and
run with no guard, as they do in the reference.

**All three variants, in one change.** They differ in nothing but a guard threshold and a measured profile. The
precedent is `add-upscale-tokyo-saitama` and `add-light-adjustment-paris-lyon`.

**Gothenburg's and Malmö's measured provider tuning**, currently `EpProfile::default()` for both: CoreML restricted to
the CPU and GPU, at **FP16 only**, ported from the reference.

**Stockholm's provider tuning, measured here.** Nobody has measured it, in the reference or here, and it is NAFNet
rather than Restormer, so the other two's findings say nothing about it. It is swept on Apple Silicon against the CPU
provider and CoreML, and its profile is whatever that supports, including the default if nothing beats it.

### Deliberate divergence from the reference

- **A guarded tile keeps its input at the run's own depth.** The reference returns the tile as an 8-bit `image.RGBA`, so
  on a 16-bit run a guarded tile is quantised to 8 bits inside an otherwise 16-bit result. Here the input tensor is
  copied into the output tensor and decoded like any other tile, so the kept pixels are the input's at whatever depth
  the caller asked for. At 8 bits the two are the same.
- **Progress reaches the end after the blend, not after the last tile.** The reference's driver reports 1 on the last
  tile and the full-resolution blend then runs after a bar that already says finished. Here the tiles report across
  most of the operation's share and the blend reports the rest.
- **Gothenburg's and Malmö's profiles are ported, not re-measured.** Paris and Lyon re-confirmed the reference's
  figures on Apple Silicon before adopting them; this change takes these two as the reference recorded them. The
  blast radius of a wrong one is a slower Mac at FP16 rather than a wrong image, and each model's prose says where its
  numbers came from.

### Infrastructure: what `rust-sak` already covers

**Nothing is needed from it, and nothing here is missing from it.** The tile grid, the tensor conversions, the blend and
the session seam are all in `inference/`; the guard is a scan over a buffer this crate already owns. `crypto`, `fetch`,
`fs`, `image`, `memo` and `sysinfo` have no bearing on any of it. **No new third-party crate.**

### Task estimate

**13.** The pipeline and its fake-backend suite, the guard and its tests, three model directories, the two variant
seams, the family seam, the refusal arm and its inverted tests, the prose sites that list what runs, `perf`'s declined
example, the live check, Stockholm's provider sweep, and the toolchain pass. One change, not a series.

## Capabilities

### New Capabilities

- `denoise`: what a denoise run does to a photograph — that it keeps the photograph's resolution, what the per-run
  strength does at 0, at 1 and past it, what the divergence guard keeps and for which model, what a run reports and
  stops on, and what it refuses.

### Modified Capabilities

- `execution-providers`: Gothenburg's and Malmö's measured settings join the pinned per-model requirements, both
  FP16-only, and Stockholm's are measured on this project's build and pinned whatever they turn out to be.

`image-enhancement` and `inference-tiling` get **no delta**. The refusal requirement is written generically and names
no family, and nothing in the tiling capability moves: the guard is applied by the denoise pipeline to the tile it is
handed, not by the driver.

## Impact

**Crates: `opai`, plus one test fixture in `perf`.** No `gui`, no `cli`. Neither front end draws a denoise control that
reaches a run yet, and every line here is library logic both would need.

- `crates/opai/src/models/denoise/process.rs` — new: the pipeline, the guard, the blend.
- `crates/opai/src/models/denoise/stockholm/`, `.../gothenburg/`, `.../malmo/` — new: each variant's guard and profile,
  the prose behind it, and the tests asserting it.
- `crates/opai/src/models/denoise/variant.rs` — `guard()`, and `profile()` reaching the model directories.
- `crates/opai/src/models/denoise/mod.rs` — the family's contract seam.
- `crates/opai/src/models/operation.rs` — denoise leaves the refusal arm; the cross-family tests that name the refused
  families and the measured variants move.
- `crates/opai/src/lib.rs` — `Opai::process`'s "What is refused", already stale since colour balance landed.
- `crates/perf/src/{main,json,report}.rs` — the tests using Stockholm as their example of a declined model move to a
  family that is still refused. The outcomes are fabricated, so they pass either way; the example would just be false.

**Public API: additive.** `Opai::process` gains six operations it can serve. No signature changes, and everything the
pipeline is built from is `pub(crate)`.

**The riskiest part is that the guard cannot be shown firing on a real photograph.** No input known to blow Stockholm up
is at hand, so the threshold is the reference's, carried over on trust and pinned by a fake-backend test. The design
says so rather than implying the guard is validated.
