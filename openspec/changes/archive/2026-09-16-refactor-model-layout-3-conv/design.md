## Slices

This refactor is three changes. Slice 1 established the seam and moved no files; slices 2 and 3 are then file moves
with no dispatch edits, because after slice 1 nothing outside a model's own code names a model. **This is slice 3, the
last of the three.**

| # | Change | Delivers | Depends on | State |
| --- | --- | --- | --- | --- |
| 1 | `refactor-model-layout-1-seam` | The `Pipeline<B>` trait, both impls in place, `chain.rs` made model-blind, the `Operation` → `Pipeline` seams in `models/`, and the module-layout doctrine written into `config.yaml`. No files move. | — | **done** |
| 2 | `refactor-model-layout-2-osaka` | `inference/osaka/` disappears. `canvas.rs` is promoted to `inference/`; `mod.rs`, `latent.rs`, `region.rs`, `noise.rs`, `colorfix.rs` and `live.rs` move to `models/upscale/osaka/`. The multi-graph vocabulary (`GraphRole`, `Graph`, `DiffusionVariant`, `GraphSet`, `ResolvedGraph`, `UnknownRole`) moves to `models/upscale/osaka/graph.rs`, `OsakaPrecision` to `models/upscale/osaka/precision.rs`, and the `OSAKA` row, its profile arm and `BROKEN_OPTIMIZERS` into `osaka/mod.rs`. The `models/` and `inference/` module docs are rewritten to the new doctrine. | 1 | **done** |
| 3 | `refactor-model-layout-3-conv` | `inference/upscale.rs` becomes `models/upscale/passes.rs`; `ConvVariant`, `ScaleBucket` and `FOUR_ONLY` become `models/upscale/conv.rs`; the `TOKYO`, `KYOTO` and `SAITAMA` rows with their profiles, measurement prose and tests move into `models/upscale/{tokyo,kyoto,saitama}/`. The six exhaustive matches over `UpscaleVariant` collapse into one `model()`. `catalogue.rs` is repointed. One directory per model, done. | 2 | **current** |

Nothing follows this slice. The convolutional pipeline is the last file under `inference/` that any model owns, and
`model()` is the seam slice 1's D2 specified and slice 2 could not build because three of the four models had not
moved.

## Context

See `proposal.md` — Why. What matters for the approach:

- **Slice 1 removed every dispatch site that named a model, and slice 2 proved the move is then cheap.**
  `chain.rs` holds `Arc<dyn Pipeline<B>>`; `Operation::pipeline` knows only families; `Upscale::pipeline` knows only
  contracts. This slice edits no dispatch either. What it does that slice 2 did not is build seam 1, which is new code
  rather than a move.
- **The six matches are not six decisions.** `profile`, `label`, `codename`, `conv_row`, `diffusion_row` and
  `is_diffusion` each answer one question per model; four of them are already one-line delegations into `osaka/` for
  their `Self::Osaka(_)` arm, which slice 2's D3 called the deliberate one-slice cost this slice retires.
- **`precision()` is not one of the six.** It matches over the payload each arm carries — `FloatPrecision` for three
  arms, `OsakaPrecision` for one — and widens it. That is a question about the precision domain a variant is published
  in, not about the model, and the model cannot answer it because a `&'static dyn` has no precision.
- **`ConvVariant`'s three rows differ in exactly two things: the row's data and the profile measured for it.**
  `variant.rs`'s own header states the first half — "A row per variant rather than a type per variant: everything that
  differs between Kyoto and Saitama is a codename, a display label and a table". The profile is the half that header
  predates.
- **The measurement prose is the expensive part.** Tokyo's arm alone carries the ANE-compiler account, two measured
  tables (`-23%`/`+3.9%` on CoreML at the two precisions, `-24.2%`/`+3.3%` on the CPU provider against Kyoto) and five
  recorded rejections with figures. Kyoto's carries the FP32-islands lesson and the 25%-at-4x figure; Saitama's the
  `-17.6%` re-export comparison; and a shared section argues why the FP32 arms are load-bearing rather than a
  formality. None of it is load-bearing to the compiler.
- **The fake-backend suite is what makes this verifiable.** Pass sequencing, overshoot correction, depth preservation,
  progress monotonicity, bucket ordering and coverage, artifact composition, identity and cache tags are all
  arithmetic against a counting fake, on every platform, with no ONNX Runtime and no weights. **No test may be lost.**
- **Four of the touched names are public API.** `Pass`, `Resolution`, `UpscaleVariant` and `Upscale`'s
  `resolve`/`graph`/`params` are re-exported at the crate root and exercised by `crates/opai/tests/public_api.rs`,
  which imports them from `opai::` by name. Nothing about their paths may move.
- **`catalogue.rs` is the one cross-family file that still names models.** It imports `KYOTO`, `OSAKA`, `SAITAMA` and
  `TOKYO` and reaches `super::upscale::variant::ConvVariant` by path.

## Goals / Non-Goals

**Goals:**

- `models/upscale/{tokyo,kyoto,saitama,osaka}/` is the whole of each model: its row, its measured profile and the
  prose behind it, and its tests. Deleting a model is deleting its directory, its `UpscaleVariant` arm and its
  `model()` arm — and nothing in `catalogue.rs`, `resolve.rs` or `conv.rs`.
- `inference/` holds nothing any model or any family owns: the tile grid, padding, tensor conversion, both blending
  strategies, the session seam, `Pipeline` and the chain driver.
- One match from the variant enum to `&'static dyn UpscaleModel`, one match from `Resolution` to a pipeline. Those two
  are the only places a model is named for dispatch, and neither is in `inference/`.
- Every public path, every error message, every measured constant and every line of measurement prose survives
  byte-for-byte.
- The test count does not decrease, in the fake-backend suite or in the live one.

**Non-Goals:**

- Touching the seven non-upscale families. They have one contract, one row shape (`SimpleVariant`) and no measured
  profile, so there is no second model to promote anything for and no match worth collapsing. Whether they eventually
  grow an `<Family>Model` seam is a question for the change that gives one of them a pipeline.
- Any change to `Backend`, to `Pipeline<B>`, or to the way sessions are acquired, held or released.
- Any change to a measured EP profile, a session option, a bucket table, a constant or the prose recording how it was
  arrived at. A value that changes in this slice is a bug in the move.
- Moving `PassWeights` out of `inference/progress.rs`, or splitting `inference/live.rs`. See D6.
- Promoting anything out of `osaka/`. The five files slices 1 and 2 listed as known candidates still have exactly one
  caller each; the promotion rule fires on a second caller, not on a prediction.

## Decisions

### D1 — `UpscaleModel` has one implementation per **contract**, not one per model

The trait slice 1's D2 specified is deliberately non-generic, so it is object-safe and usable as a `&'static dyn`:

```rust
pub(crate) trait UpscaleModel: Send + Sync {
    fn codename(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn precisions(&self) -> &'static [Precision];
    fn profile(&self, precision: Precision) -> EpProfile;
    fn is_diffusion(&self) -> bool;
    fn resolve(&self, precision: Precision, scale: Scale) -> Resolution;
    fn graph(&self, precision: Precision, role: GraphRole) -> Result<ArtifactId, UnknownRole>;
}
```

It is implemented **twice**: once for `ConvVariant` in `models/upscale/conv.rs`, once for `DiffusionVariant` in
`models/upscale/osaka/graph.rs`. Four models, two impls — because every difference between Tokyo, Kyoto and Saitama is
data in their rows, and every difference between those three and Osaka is the contract.

That is the shape `variant.rs`'s own header already argues for: *a row per variant rather than a type per variant*. A
unit struct and a trait impl per model would put four near-identical bodies in four directories, each delegating five
of seven methods to a family-tier function with its own row — which is the copying the doctrine names as one of the
two things that are never the answer to a second caller.

The three answers the two impls give that are worth stating, because they are contract facts rather than per-model
ones:

- `is_diffusion` is `false` for every `ConvVariant` and `true` for every `DiffusionVariant`. Whether the requested
  scale selects the weights *is* the contract, which is what the method was always asking; a per-model answer would
  have let two models of one contract disagree about it.
- `resolve` walks the bucket table for `ConvVariant` (the body that leaves `Upscale::resolve`) and calls
  `graph::resolve` for `DiffusionVariant` (the body slice 2 already moved there).
- `graph` is `Err(UnknownRole { role, variant: self.codename() })` for every `ConvVariant` — the refusal
  `resolve.rs` produces today, written once for the contract that declares no graphs rather than three times — and
  `graph::artifact_for` for `DiffusionVariant`.

Rejected: **a unit struct per model** (`tokyo::Model`, `kyoto::Model`, …) with seven methods each. It reads closer to
"one directory per model" at a glance, and it is four times the boilerplate for one real difference. Rejected also: a
**blanket** `impl<M: ConvModel> UpscaleModel for M`, which does not compile beside `impl UpscaleModel for
DiffusionVariant` — coherence cannot rule out a `DiffusionVariant: ConvModel` impl, so the two overlap.

### D2 — Seam 1 is a match to a `static`, and the family keeps an ordered list beside it

```rust
// models/upscale/variant.rs — SEAM 1, one arm per MODEL
pub(crate) fn model(self) -> &'static dyn UpscaleModel {
    match self {
        Self::Tokyo(_)   => &tokyo::TOKYO,
        Self::Kyoto(_)   => &kyoto::KYOTO,
        Self::Saitama(_) => &saitama::SAITAMA,
        Self::Osaka(_)   => &osaka::OSAKA,
    }
}
```

The four rows become `static`s rather than `const`s, so `&'static dyn` is a borrow of a known address rather than a
const promotion the reader has to reason about. `label()`, `codename()`, `profile()` and `is_diffusion()` stay on
`UpscaleVariant` as one-line forwarders through `model()`, so every existing caller — `Upscale::profile`,
`Display for Upscale`, `params()`, `PartialEq`, `Hash`, `cache_tag()`, and the tests — keeps the call it makes today.
The six *matches* collapse; the methods do not disappear. `conv_row()` and `diffusion_row()` do disappear: their only
non-test callers were `Upscale::resolve` and `graph::resolve`/`artifact_for`, all of which now reach the model
directly.

Exhaustiveness is preserved and is still what the design rests on: a variant added to the enum without a directory
does not compile, because `model()` has no arm for it. A row written without a profile does not compile either, since
`profile` is a required field of the row rather than an arm someone could forget — which is the same guarantee
`variant.rs` argues for today, moved from the match to the struct.

Beside the seam, the family holds one ordered list:

```rust
// models/upscale/mod.rs
pub(crate) static MODELS: [&'static dyn UpscaleModel; 4] = [&tokyo::TOKYO, &kyoto::KYOTO, &saitama::SAITAMA, &osaka::OSAKA];
```

It is not a second dispatch seam — nothing resolves through it — it is the *enumeration* the catalogue needs, in the
order a chooser offers the variants, which is the order it has today. The seven simple families already expose exactly
this (`denoise::ALL` and its six siblings), and D4 is why the catalogue must read it instead of importing rows. A test
drives `every_variant()` and asserts each variant's `model()` is in `MODELS`, so a model added to seam 1 and forgotten
in the list fails rather than being silently absent from every chooser.

Rejected: deriving the list from the enum instead. `UpscaleVariant` cannot be enumerated without supplying a
precision per arm, and the precisions differ by arm — which is the whole point of the precision living inside the
variant.

### D3 — The measured profile travels in the row, as a `fn(Precision) -> EpProfile`

```rust
pub(crate) struct ConvVariant {
    pub(crate) codename: &'static str,
    pub(crate) label: &'static str,
    pub(crate) buckets: &'static [ScaleBucket],
    /// The execution-provider tuning measured for this model. See the model's own directory for the measurements.
    pub(crate) profile: fn(Precision) -> EpProfile,
}
```

`DiffusionVariant` gains the same field. Each model's directory then holds its row and the function the row points at,
with all of that model's prose on the function — which is what makes the directory the whole of the model.

**This is the signature the reference implementation uses, and it is not the same decision.** `variant.rs`'s profile
comment records why the reference needs `func(Precision) EPProfile` *and* an `Fp16Only` wrapper: a Go variant cannot
carry its own precision, so the function is how the precision reaches the measurement at all and the wrapper is how a
measurement that holds at one precision is kept from being applied at the other. Here the precision is inside the
variant, the pairing that wrapper enforces is enforced by construction, and the function carries something else
entirely: which model's measurement this is. No wrapper is adopted, and no arm gains a guard.

What is genuinely given up is one narrowing. `UpscaleVariant::profile`'s arms are written today against
`FloatPrecision::Fp16` and `OsakaPrecision`, and a `&'static dyn` cannot be handed a narrow precision — so Kyoto's and
Saitama's functions match `Precision::Fp16` with a fall-through, where the match arm had `FloatPrecision::Fp16` and
`_`. The fall-through was already there; what is lost is that the compiler could previously see there were only two
float precisions to fall through to. The `no_variant_beyond_the_four_measured_arms_declares_anything` test, which is
driven over every published pairing rather than transcribed, is what already covers the case the narrowing covered,
and it stays in `variant.rs`. It is driven from `every_variant()`, so its subject is every pairing the family
publishes — the two Osaka ones included, which `conv.rs` cannot see. A test over all four models belongs beside the
enum that enumerates them, not inside one of the two contracts.

Rejected: **keeping `profile` a match in `variant.rs`**, with only the rows and prose moving. It leaves one of the six
matches standing, which is the thing the slice exists to remove, and it leaves Tokyo's deletion touching `variant.rs`
twice. Rejected also: **`profile: EpProfile` plus `fp16_profile: Option<EpProfile>`** — that is the `Fp16Only` wrapper
spelled as two fields, and this project already declined it.

### D4 — `catalogue.rs` stops naming models, and each precision domain publishes its own widened list

The doctrine's success criterion is that deleting a model is its directory plus two arms. `catalogue.rs` imports four
rows today, so deleting Tokyo would edit a cross-family file as well. It is repointed to build the upscale entry from
`upscale::MODELS`, asking each model for its codename, its label and its precisions.

That last one is why the trait carries `precisions()`. Today the catalogue special-cases it: three conv rows get
`FLOAT_PRECISIONS`, Osaka gets `OSAKA_PRECISIONS`, and which is which is decided *outside* the models — the exact
shape of drift the catalogue exists to prevent, and one that would be silently wrong the moment a second diffusion
model published a different pair. So the two widened lists move to sit with the domains they widen:
`FloatPrecision::PRECISIONS` in `models/precision.rs` and `OsakaPrecision::PRECISIONS` in
`models/upscale/osaka/precision.rs`. `catalogue.rs`'s `simple_family` reads the first for its seven families,
`impl UpscaleModel for ConvVariant` reads the first, `impl UpscaleModel for DiffusionVariant` reads the second, and
the two `LazyLock`s in `catalogue.rs` go away.

The existing catalogue tests pin the order of the four entries and the exact precision list each carries, so the
repoint is checked by tests that are not edited.

Rejected: leaving the special case and keying it on `is_diffusion()`. It compiles, it is one line, and it is a lookup
beside the model rather than from it — which is the same defect `ResolvedGraph` exists to prevent one layer down.

### D5 — `passes.rs` is family tier and `conv.rs` is family tier; neither is one model's

Two files under `models/upscale/` rather than inside a model's directory, because each has three callers:

| File | What it holds | Why not model tier |
| --- | --- | --- |
| `passes.rs` | `PassSequence`, `run_passes`, `correct_overshoot`, `RunPass` | Tokyo, Kyoto and Saitama all run it, unchanged. The doctrine names this file by name as its family-tier example. |
| `conv.rs` | `ScaleBucket`, `ConvVariant`, `FOUR_ONLY`, the bucket walk, `impl UpscaleModel for ConvVariant` | Three rows are built from it and `FOUR_ONLY` is literally shared by two of them. |

`FOUR_ONLY` is the sharpest case: it is Tokyo's table and Saitama's table, one value. Parking it in either directory
would make the other's row import across a model boundary, which the doctrine calls a promotion that has not happened
yet. It is promoted here, in the change that creates the second caller, which is the rule's own timing.

Each model's directory is a directory with one file (`mod.rs`) rather than a `tokyo.rs` beside `conv.rs`. That is not
ceremony: the doctrine distinguishes a model (`models/<family>/<model>/`) from a family-tier file
(`models/<family>/<file>.rs`) by exactly that, so `tokyo.rs` and `passes.rs` sitting side by side would make the tier
of each unreadable from the tree.

### D6 — What stays in `inference/`, and why naming a model is not the same as being one

Two things under `inference/` name Tokyo or Kyoto after this slice, and both stay:

**`PassWeights`, in `inference/progress.rs`.** After the move its only caller is `passes.rs`, which reads as a
demotion candidate. It stays because its subject is the chain's progress composition, not the upscale family's: it is
one of the three weightings that module's header is about, and the test that matters for it —
`the_chain_fraction_never_decreases_across_a_multi_operation_multi_pass_run` — drives `ChainProgress` and
`PassWeights` together, because monotonicity across an install handing over to a multi-pass run is a property of the
two composed. Moving the type would either split that test across two tiers or drag `ChainProgress` down with it.
Nothing about it is a property of a convolutional upscaler; it weights passes by input pixel area, which is what any
multi-pass operation would want.

**`inference/live.rs`.** It runs Kyoto end to end through `Opai::process`, and its last two tests run one Tokyo tile
in each execution mode. Its subject is the chain, the session cache and the provider contract — it is the live
counterpart of the fake-backend suite in the same directory, and Kyoto and Tokyo are the smallest real graphs
available to drive them, exactly as that suite uses a counting fake. The sequential-mode pair in particular checks a
requirement of the `execution-providers` spec (that declaring sequential execution does not change the pixels); moving
it into `tokyo/` would file a provider requirement under one model.

The contrast that makes this a rule rather than an exception is slice 2: `osaka/live.rs` moved because its subject was
Osaka's own pipeline — region packing, three graphs, the colour fix. **A test that names a model is not a test of that
model.** The tier question is what a file covers, not what it mentions.

## Division across crates

`opai` only. `gui` and `cli` never name the upscale family; `perf` reaches it through `catalogue()` and
`Operation::from_catalogue_with`, which keep their shapes and their output — `MODELS` is `pub(crate)` and the
catalogue is what crosses the crate boundary, as it does today. No logic moves toward a front end, and nothing a front
end would need is introduced.

## Dependencies

**`rust-sak`:** no feature added or dropped, and nothing here is the kind of thing it covers. This slice writes no
infrastructure — no hashing, HTTP, retries, downloads, extraction, temp files, config directories, image codecs,
caching, telemetry or hardware probes. Its only new code is one trait and two implementations of it over data that
already exists. `rust-sak`'s existing use in this crate (`crypto` and `memo` through `cache.rs`, `image` through the
loader) is on no path this slice edits.

**Other crates:** none introduced, and none considered. `image` and `image::imageops::FilterType` travel with
`run_passes` and `correct_overshoot` into `passes.rs` and keep their existing declarations.

## Ownership and lifetime

Unchanged, and the trait is shaped to keep it that way. `UpscaleModel` answers **data** questions only — names,
precisions, a profile value, a `Resolution`, an `ArtifactId`. It is not generic over `Backend`, holds no session,
takes no `SessionHandle` and returns no pipeline. The backend-generic half stays where slice 1 put it: seam 2 in
`models/upscale/mod.rs` matches `Resolution` and constructs `passes::PassSequence` or `osaka::Diffusion`, both of
which remain plain-data `Pipeline<B>` implementations that borrow `&[SessionHandle<B::Session>]` for the duration of
`run` and cannot retain, drop or reorder one.

`&'static dyn UpscaleModel` introduces no ownership at all: the four rows are `static`s in the binary, so there is no
`Arc`, no reference count and no lifetime shorter than the process. The chain still acquires every handle for an
operation before any of it runs and holds all of them until the whole chain returns.

The one ownership-adjacent thing this slice could break is the pairing of an artifact with the profile it was measured
under. `PassSequence` carries one profile for its whole sequence and `Diffusion` carries one per graph, and
`Pipeline::sessions` hands out `(&ArtifactId, &EpProfile)` pairs already made — none of which this slice touches. The
verification is that the existing seam tests pass unedited: the one asserting a pass sequence carries one profile
while a graph set carries one per graph, and the chain tests asserting a session a run holds cannot be reclaimed by
the idle sweep, an explicit release or a provider switch.

## Platforms and execution providers

Validated by the existing fake-backend suite, which is arithmetic and runs on macOS, Windows and Linux with no ONNX
Runtime, no GPU, no model file and no network — so every property it covers is checked everywhere. Beyond it, the
`#[ignore]`d live suites exercise real graphs on macOS/Apple Silicon, CoreML and CPU, which is the hardware available
here.

**CUDA and TensorRT are untested for this slice**, as they were for the changes that measured the profiles being
moved. That is acceptable for the same reason as in slice 2: no provider-selection code, no session-building code and
no EP profile value is touched. Tokyo's `CPUAndGpu`/`Sequential`, Kyoto's and Saitama's `CpuAndNeuralEngine`, and
Osaka's `cuda_prefer_nhwc` overrides and `trt_builder_optimization_level` move as literals and are asserted by tests
that name their values, so a CUDA or TensorRT machine would find them exactly as it would today.

## Risks / Trade-offs

- **Measurement prose is summarised, reflowed or dropped in the move — the single largest risk in the slice.** Three
  models' worth of tables, transcripts and recorded rejections, none of it load-bearing to the compiler, so every test
  would still pass. → One task per model, each verified by diffing the moved comment text against `HEAD` character for
  character rather than by reading the result. Same discipline as slice 2's task 4.2, which is where this comes from.
- **A test is lost because four `#[cfg(test)] mod tests` blocks are split across six files.** `variant.rs`'s block
  alone holds bucket-ordering, coverage, codename, round-trip and four profile tests, and they land in `conv.rs`,
  three model directories and `variant.rs`. → The test count is compared before and after with
  `cargo test -p opai -- --list`, and no assertion may be dropped rather than repointed.
- **`ConvVariant` gaining a `fn` pointer is read later as the reference's `func(Precision) EPProfile` having been
  adopted wholesale, and the `Fp16Only` wrapper follows it in.** → D3 is written to be read by that person, and the
  distinction is repeated on the field's own doc comment rather than left in the planning artifacts.
- **Kyoto's and Saitama's profile functions match a widened `Precision` with a fall-through, so a third float
  precision would silently take the default.** → Real but small, and pre-existing in substance: the `_` arm is already
  there. `no_variant_beyond_the_four_measured_arms_declares_anything` is driven over every published pairing rather
  than transcribed, so a pairing that acquired a profile nobody measured fails there.
- **A public path moves without anyone noticing.** `tests/public_api.rs` imports from `opai::` by name and would
  still compile if a re-export were repointed through a different module. → It would still compile, which is the
  point: the re-export chain may change, the paths may not. Verified by diffing `cargo public-api` (or `cargo doc`'s
  index) against `HEAD`, not by the suite alone.
- **`cargo doc` degrades quietly.** `models/simple.rs`, `osaka/graph.rs` and `osaka/latent.rs` carry intra-doc links
  to `super::upscale::variant::ConvVariant` and `crate::inference::upscale`, and broken intra-doc links are warnings
  here, not errors. → A task builds the docs and requires zero new rustdoc warnings against `HEAD`.
- **`git` records the moves as delete-plus-add rather than as renames, because files are edited in the same commit
  they move in.** → The mechanical move and the edits that follow it are separate tasks, so `git log --follow` and a
  rename-detecting diff still work on `passes.rs`, the one file that moves whole.

## Open Questions

None. What moves where was settled by slice 1's table. The three questions this slice had to answer on its own — how
many implementations of `UpscaleModel` there should be (D1), where a model's measured profile lives once a `dyn`
cannot be handed a narrow precision (D3), and what a cross-family file may still name (D4, D6) — are each decided
above with the alternative that was rejected.
