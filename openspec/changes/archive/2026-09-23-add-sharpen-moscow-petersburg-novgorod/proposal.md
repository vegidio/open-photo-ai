## Why

Moscow, Petersburg and Novgorod can be named, parameterised, catalogued, downloaded, opened and cached, and they cannot
run. `Sharpen`, `SharpenVariant`, `SharpenParams`, `Strength`, the three constructors, the catalogue rows, the artifact
names, the run-cache tag and the serde form all ship already. The one thing between a user and a sharpened photograph is
`Self::Sharpen(_)` in `Operation::pipeline`'s refusal arm.

The contract it needs already exists as well, inside the wrong tier. The reference's `sharpen/process.go` and
`sharpen/variant.go` are denoise's files line for line with the prefix changed: a tile grid at scale 1, an optional
per-tile divergence guard, then the strength blend. `models/denoise/process.rs` is that contract, and both its own
header and the archived denoise design name this change as the one that promotes it. The tier rule moves a file in the
change that introduces its second caller, before that caller is written, and forbids a copy.

## What Changes

**The contract moves to the cross-family tier, unchanged in behaviour.**

- `models/denoise/process.rs`'s guard check, its progress share and its pipeline move to a new `inference/filter.rs`,
  shared by denoise and sharpen. They take the range and the `Strength` from their caller, not from `DenoiseParams`
  and denoise's own range constant, so neither family's vocabulary leaks into the shared file.
- Each family keeps what is its own: its range, which variants are guarded, the measured profiles, and its seam.
- Denoise renders exactly what it renders today. Its fake-backend suite passes with nothing changed but imports and the
  way the pipeline is constructed, and its live check is re-run against the real weights.

**The sharpen family runs.**

- `Sharpen::pipeline`, the family's contract seam, and the `Operation::pipeline` arm that reaches it. Sharpen leaves
  the refusal arm, and colorization is left in it alone.
- The per-run `Strength` is applied **after** the tile grid and never reaches the graph, so one session serves every
  position of a slider. Above 1 it extrapolates past the model's output, as the reference does.

**The divergence guard, for Petersburg alone.** Petersburg is the NAFNet model in this family, as Stockholm is in
denoise. Where any raw output value of a tile exceeds 3.0 in magnitude, that tile keeps its input pixels. Moscow and
Novgorod are Restormer models and run with no guard, as they do in the reference.

**All three variants, in one change**, as `add-denoise-stockholm-gothenburg-malmo` shipped its three.

**All three provider profiles, ported from the reference and not re-measured.**

- Moscow and Novgorod: CoreML restricted to the CPU and GPU, at **FP16 only**.
- Petersburg: the provider defaults at both precisions, which is a **measured** outcome in the reference: CoreML already
  places the whole graph on the GPU, so no candidate beat the default.

### Deliberate divergence from the reference

The same three differences denoise already has. The contract is shared, so sharpen inherits all three at once:

- **A guarded tile keeps its input at the run's own depth.** The reference keeps an 8-bit copy of the tile, which
  quantises a guarded region inside a 16-bit result. At 8 bits the two are the same.
- **Progress reaches the end after the blend, not after the last tile.** The reference reports finished before its
  full-resolution blend has run.
- **The profiles are ported, not re-measured.** A wrong one gives a slower Mac at FP16, never a wrong image, and each
  model's prose says where its numbers came from.

The label stays **Petersburg**, not "St. Petersburg". That is the label the reference shows, and it is already pinned
in the catalogue.

### Infrastructure: what `rust-sak` already covers

**Nothing is needed from it, and nothing here is missing from it.** The tile grid, the tensor conversions, the blend and
the session seam are in `inference/`, and the guard is a scan over a buffer this crate already owns. **No new
third-party crate.**

### Task estimate

**14.** Five for the promotion: the move, the tests that go with it, rewiring denoise, the prose describing the old
tiers, and denoise's live check. Nine for sharpen: three model directories, the variant matches, the family seam and
refusal arm, the prose and tests that name sharpen as refused, the live check, and the toolchain pass. One change, not a
series. The canvas promotion was split into its own slice because it would have made 21 tasks; this one fits.

## Capabilities

### New Capabilities

- `sharpen`: what a sharpen run does to a photograph. It keeps the photograph's resolution. It covers what the per-run
  strength does at 0, at 1 and past it, what the divergence guard keeps and for which model, what a run reports and
  stops on, and what it refuses.

### Modified Capabilities

- `execution-providers`: Moscow's, Petersburg's and Novgorod's measured settings join the pinned per-model requirements.
  Moscow and Novgorod are FP16-only, and Petersburg is a measured default at both precisions.

`denoise` gets **no delta**: moving its code changes nothing it does, and its unchanged suite is the proof.
`image-enhancement` and `inference-tiling` get none either. The refusal requirement names no family, and the guard is
still applied by the pipeline to the tile it is handed, not by the tiling driver.

## Impact

**Crates: `opai` only.** No `gui`, no `cli`, no `perf`. The GUI's sharpen control is built from the catalogue and
reaches a run through `Opai::process` with nothing to change, as denoise's did. `perf` already selects sharpen variants
by codename, and its declined-model examples moved to colorization with denoise.

- `crates/opai/src/inference/filter.rs`: new, holding the guard check, the progress share and the tiled-then-blended
  pipeline, with the fake-backend suite that covers them.
- `crates/opai/src/models/denoise/process.rs`: reduced to denoise's range and a thin call into the shared pipeline, or
  removed if nothing of denoise's own is left in it (design D1).
- `crates/opai/src/models/sharpen/moscow/`, `.../petersburg/`, `.../novgorod/`: new, holding each variant's guard and
  profile, the prose behind them, and the tests asserting them.
- `crates/opai/src/models/sharpen/{mod,variant}.rs`: the family seam, `guard()`, and `profile()` reaching the model
  directories.
- `crates/opai/src/models/operation.rs`, `inference/process.rs`, `autopilot.rs`: sharpen leaves the refusal arm, and
  the tests and doc comments that use it as the example of a refused family move to colorization.
- `crates/opai/src/lib.rs`, `inference/mod.rs`, `inference/mix.rs`: prose naming what runs, and where the guard lives.

**Public API: additive.** `Opai::process` gains six operations it can serve. No signature changes, everything moved is
`pub(crate)`, and `tests/public_api.rs` is untouched.

**The riskiest part is the move itself**, because every way it can go wrong produces a plausible photograph and no
error. Denoise's suite passing with no change to any assertion, and its live check against the real weights, are what
show the move was only a move.
