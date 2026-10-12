## Context

See proposal.md — Why. What matters here is the shape of what already exists.

The upscale family's vocabulary for Osaka is complete and unused: `UpscaleVariant::Osaka`,
`DiffusionVariant`, `GraphRole`, `Resolution::Graphs(GraphSet)`, the FP16 pins on the VAE halves, the
hand-written `PartialEq`/`Hash` that make two Osaka scales one model, and `UpscaleParams { scale }`
for the scale that travels per run. `Normalisation::Signed` — the `[-1,1]` range Osaka needs — sits in
`inference/tensor.rs` under an `expect(dead_code)` whose stated reason is that its first caller has
not arrived. This slice is that caller.

Three things in `inference/` are shaped for the convolutional contract and only for it:

- `inference::session::run_tile` recovers its tensor shape from the buffer — `sqrt(input.len() / 3)`.
- `chain::Planned` holds `passes: Vec<Pass>` and one `profile: EpProfile` for all of them.
- `chain::plan_one` refuses `Resolution::Graphs` outright.

## Goals / Non-Goals

**Goals**

- Answer the three questions that cannot be answered without the real model: does the transformer
  load under the pinned runtime, is the scheduler step read correctly, is the 33-channel packing
  order right.
- Land Osaka's measured provider settings, including the first per-graph override in the project.
- Widen the inference seam to shapes that are not three-channel and not square, without disturbing
  the path every other model takes.

**Non-Goals**

- Running Osaka from the public API. The refusal stays up for the whole of this slice — see D8.
- Anything at image scale: the Lanczos resample to target, the tile grid over a whole image, the
  blending accumulator and the wavelet colour fix are all slice 2.
- A memory budget. See D9.
- Bit-parity with the Go application's output. See D6.

## Slices

Two slices. **This change is slice 1.**

| # | Name | Delivers | Depends on | State |
|---|------|----------|-----------|-------|
| 1 | `add-upscale-osaka-1-region` | The provider settings, the shaped inference seam, the `Planned` split, and one 960x960 region restored end to end against the real model. Osaka stays refused through the public API. | Nothing beyond what is merged | **current** |
| 2 | `add-upscale-osaka-2-pipeline` | The Lanczos resample to target size, the tile grid over the whole image, the weighted float-CHW canvas, the wavelet colour fix, depth and progress — and lifting the refusal. Osaka works. | Slice 1 | pending |

Slice 2 is not proposed until this one archives. Its estimated task count is 12.

## Decisions

### D1. Slice by risk, not by what looks tidiest

The obvious cut is arithmetic first — the canvas and the colour fix are pure, self-contained and need
no model — then wiring. It is the wrong way round.

The canvas and the colour fix are the two pieces here that **cannot be silently wrong**. They are
deterministic arithmetic over float buffers, and a unit test either passes or does not. The three
things that *can* be silently wrong all live in this slice, and none of them is reachable without the
real weights:

1. **Does the transformer load at all?** The reference records that ONNX Runtime 1.26 rewrites this
   graph so that it refers to a tensor it removed, failing session creation with a name that does not
   exist, and that `ReshapeFusion` and `SimplifiedLayerNormFusion` must be off. That is a claim about
   a Go ORT build; this project reaches 1.26 through `ort` 2.0.0-rc.13.
2. **Is the scheduler step read correctly?** The graph's output is named `denoised_latent` and is the
   flow-matching velocity, not the latent. For a single step the schedule collapses to
   `x0 = noise - prediction`. The reference is explicit that getting this wrong is not subtle — the
   decode returns the image buried under the velocity field — but it still *returns an image*.
3. **Is the 33-channel order right?** `[noise | cond | ones]`. All 33 channels go through a single
   projection, so nothing in the graph's structure says which group is which, and a wrong order
   returns a worse image rather than an error.

An arithmetic-first slice would spend a whole change on the safe code and discover all three in the
second. This ordering pays for the download once and answers them first.

**Alternative considered — three slices, splitting the seam (`run_graph`, the `Planned` split, the
profiles) from the region.** Rejected. That first slice would deliver nothing checkable at all, not
even against a test, and the second would still be where every unknown landed. Two slices with the
risk in the first is strictly better than three with the risk in the middle.

### D2. `run_graph` beside `run_tile`, not instead of it

`run_tile` stays exactly as it is. The new call takes the input shape and the expected output shape
explicitly and is used only by the Osaka region.

Collapsing the two — giving `run_tile` shape parameters and having the pass pipeline pass
`[1, 3, shape, shape]` — was considered and rejected. `run_tile`'s shape recovery is not a shortcut;
its comment records it as deliberate, *"derived from the buffer rather than passed in, so it cannot
disagree with the geometry the driver actually partitioned at"*. For a driver that allocates the
scratch at exactly the tile shape, inference from the buffer is the stronger guarantee, and replacing
it with a parameter would convert a property the compiler keeps into one a caller can get wrong.

They are two contracts, so they are two functions. `Backend` gains a second method rather than
changing its first, which also means every existing test double keeps compiling.

`run_graph` keeps the three properties `run_tile`'s doc comment defends: the input is borrowed
through `TensorRef::from_array_view` rather than copied, the output is copied once out of the
runtime's own buffer, and the session lock is taken per call rather than held across a run. A region
therefore takes three short locks rather than one long one — which preserves the interleaving that
lets an export queue's second image make progress against the first.

### D3. The per-graph profile travels with the artifact

`Planned` becomes an enum over the two contracts `Resolution` already distinguishes:

```
  Planned::Passes { passes: Vec<Pass>, profile: EpProfile, requested: Scale }
  Planned::Graphs { graphs: Vec<(GraphRole, ArtifactId, EpProfile)> }
```

The profile sits **inside** the graph tuple rather than beside the list, for exactly the reason
`GraphSet` already binds artifacts by role rather than by position: a graph opened under another
graph's configuration produces a working session and a wrong result, with nothing downstream able to
detect it. Making the settings something a caller looks up separately is making them something a
caller can look up wrongly.

**Alternative considered — a `profile_for(role)` method on the variant**, mirroring the reference's
`GraphSpec.Profile` function pointer. Rejected: it keeps the artifact and its settings in two places
joined by a convention, which is the failure mode above.

The convolutional branch keeps one shared profile, because its passes genuinely share one — Kyoto at
8x is two passes of two artifacts from one measured declaration. Flattening both branches into
per-artifact profiles would imply a distinction the convolutional contract does not have.

### D4. Osaka's profile is transcribed, not re-derived

Every setting is taken from the reference's measurements, which were made on hardware this change
does not have. They are transcribed with their figures into the profile's own comment so the next
person can tell a measurement from a guess. The full list and its numbers are in the spec delta under
`execution-providers`; the mechanics here are only where they live.

The variant-level declaration goes in `UpscaleVariant::profile`'s `Osaka` arm, which today returns
`EpProfile::default()`. The transformer's override — `cuda_prefer_nhwc` back off — is applied where
the graph set is resolved, so that the override is a property of the graph rather than something the
chain remembers to apply.

Two `expect(dead_code)` markers fall as a result: `CoreMlComputeUnits::CpuAndGpu` is already claimed
by Tokyo, but this is the first declaration of `disabled_optimizers` and of `trt_options`, and
`clippy` fails on an `expect` that is no longer needed — which is the check that they were claimed.

**One thing deliberately not added.** The reference's `EPProfile` has seven fields this project's
does not: `Fp16`, `ExcludeEPs`, `CudaOptions`, `GraphOptimization`, `Extra`, `TrtWorkspaceBytes` and
`DynamicShapes`. A grep across the reference excluding tests shows **no model sets any of them** —
all seven are dead there. Three of the absences are load-bearing for Osaka specifically, and this
project's hardcoded defaults already produce the behaviour the reference gets by leaving them unset:
`RequireStaticInputShapes` stays on (setting `DynamicShapes` is what made CoreML fail the VAE with an
axis-range error), `trt_fp16_enable` stays off (on the INT8 export it is a real speed-up bought with
precision the caller did not ask for), and the TensorRT workspace stays at 4 GiB (24 GiB measured as
a tie). Nothing is added to `EpProfile` on this change's account.

### D5. `rand_chacha`, and deliberately not `rand`

**rust-sak has nothing for this.** Its `crypto` module is SHA-256 and XXH3-64 hashing, which is not a
generator, and no other module is close. Nor is this a gap worth filling there: a PRNG is not a
first-party concern and the ecosystem's answer is settled. The `image`, `memo`, `fetch`, `fs`,
`crypto` and `sysinfo` features this crate already enables are unchanged by this slice.

`rand_chacha` is the crate the Rust Rand Book names for this exact case. The relevant guidance is
that seeding alone does not give reproducibility — that needs a *named* algorithm with a fixed
implementation, which is why `StdRng` is wrong here: it carries no cross-version stability guarantee
and could change the noise field under a routine dependency bump, silently invalidating every cached
Osaka result. `ChaCha8Rng` is a named algorithm tested against reference vectors, and ChaCha8 is the
reference's own choice of stream, so the construction differs (see D6) while the primitive does not.

**`rand` itself is not added.** `rand_chacha` needs only `rand_core` for `SeedableRng` and
`Rng::next_u64`, and this slice converts those bits to a float itself. That is not minimalism for
its own sake: `rand`'s float sampling is a distribution implementation whose exact bit-level output is
not something this project should depend on staying fixed, and the whole point of the generator is
that its output must not move. Owning the one-line conversion makes reproducibility depend on
`rand_chacha`'s documented stream and nothing else.

**Alternative considered — a counter-based generator over rust-sak's XXH3-64**, hashing
`(seed, x, y, counter)` for each 64-bit draw. It would add no dependency at all and rust-sak is
already here with `crypto` enabled. Rejected: it is inventing a PRNG, its statistical properties for
this use are asserted rather than tested, and the saving is one small, widely-audited crate.

### D6. Determinism is preserved; bit-parity with Go is not claimed

The seed is derived from the region's origin — `(seed, origin_x, origin_y)` into a 32-byte key —
rather than from a global stream, and this is carried over deliberately. It gives two properties the
reference names and this project needs for the same reasons: a run is reproducible, so the image cache
cannot memoize a result the user is then unable to reproduce; and a tile's noise does not depend on
how many tiles preceded it, so changing the tile geometry does not reshuffle the noise of tiles that
kept their position. The second matters to slice 2, not this one, and is built in now because the
seed derivation is not something to revisit later.

What is **not** preserved is the exact stream. Go's `math/rand/v2.NewChaCha8` is the ChaCha8Rand
construction, with its own reseeding schedule, not the plain ChaCha8 stream `rand_chacha` produces.
Reproducing it would mean implementing ChaCha8Rand, and no property that matters here would be bought
by it: every use of the seed is about determinism within one implementation. The consequence is stated
plainly in proposal.md — the two applications will not produce pixel-identical Osaka output — and it
is the one behavioural divergence this slice knowingly takes.

### D7. The latent contract is written down because the graph does not carry it

**Confirmed against the published FP16 exports by task 1.1.** Every constant below is what the graphs
declare, with no symbolic dimensions on any of them — so the region size is 960x960 and nothing else:

```text
  encoder      pixel_image      [1, 3,960,960] f32  ->  latent          [1,16,120,120] f32
  transformer  vid_input        [1,33,120,120] f32  ->  denoised_latent [1,16,120,120] f32
  decoder      latent           [1,16,120,120] f32  ->  pixel_image     [1, 3,960,960] f32
```

The transformer takes **one** input and no timestep, which is what the scheduler step's collapse rests
on, and the decoder is the encoder's exact inverse, which is the VAE stride of 8.

Three constants and one packing order, all of which are conventions taken from SeedVR2's reference
implementation rather than anything discoverable from the exports: 16 latent channels, 33 transformer
channels (`2 * 16 + 1`), a VAE stride of 8, and the channel groups laid out as
`[noise | encoded_latent | ones]`. Nothing is rescaled on the way in — SeedVR2's VAE declares a
scaling factor of 0.9152 and its reference pipeline does not apply it.

These are asserted by task 1, against the actual exports, **before** anything is written against them.
If the transformer turns out to take a timestep input rather than a folded constant, or to accept a
region size other than 960, the design changes rather than the task being worked around — the
precedent is `add-upscale-tokyo-saitama` task 1.2, which found one of the reference's own node counts
wrong and corrected the design.

### D8. The refusal stays up, and slice 1 is verified by test rather than by reachability

`plan_one`'s `Resolution::Graphs` refusal is untouched by this slice. Osaka remains refused through
the public API until slice 2 lifts it.

This is the cost of D1's ordering and it is worth naming rather than glossing. The project's own rule
is that a slice should leave things *"working and testable rather than half-wired"*, and by the strict
reading this slice ends with code no public call can reach. Two things make it the right trade:

- **It is testable, which is the half that matters.** The deliverable is "one 960x960 region is
  provably restored correctly", and that is checked by a test driving the real model — a stronger
  check than public reachability would give, since a reachable-but-unverified pipeline is exactly the
  state D1 exists to avoid.
- **The project already holds code in this state on purpose.** `Normalisation::Signed` has sat under
  an `expect(dead_code)` with a stated reason since the tensor module was written, waiting for the
  first pipeline that needs it. Slice 1 claims it.

The alternative — lifting the refusal for a target size of exactly 960x960 and refusing every other
size — was rejected. It would put a size-conditional refusal in front of users to describe work in
progress, and a temporary API describing a temporary state is the thing this project does not do.

### D9. No memory-pressure warning

The reference warns before a run it expects to exhaust the pool, reading a per-pool budget from its
model registry. This project has no memory budget; `sessions/mod.rs` already records that absence and
says the twelve reference records about pools, leases and admission *"arrive with it"*. There is
nothing to read, so there is nothing honest to warn from, and a warning built on a guess about
available memory is worse than none.

Recorded as a parity gap in this design and in the archive notes rather than half-built. It is also
worth stating what it means for this slice concretely: **three sessions are resident at once** for one
Osaka operation — a 6.8 GB transformer at FP16 plus 0.17 GB of VAE halves — and nothing in this
project budgets, admits or refuses that. The failure mode on the host pool is that ONNX Runtime's
allocator takes the process down with nothing to catch it. That is unchanged from every other model
here, but Osaka is the first that makes it likely rather than theoretical.

**Measured, and it is worse than the weights suggest.** The live region checks peaked at **29.0 GB
resident** on macOS arm64 running one 960x960 region through the CPU provider — against the ~7 GB the
three graphs weigh. The gap is activations: a 12,940-node transformer at a 120x120 token grid with the
memory planner deliberately off, which is this profile's own setting. On a 64 GB machine that is
comfortable and on a 16 GB one it is not, and nothing in this project would say so before trying. The
figure is a `dev` build's and the check is a single region; slice 2's tile loop holds the same three
sessions across *every* region of an image, so the peak does not come down.

That makes this the strongest argument yet for the memory budget deferred past this series — stronger
than the 1.72 GB an 8x Kyoto run was already recorded at. It stays deferred here, because a budget is
its own change and a guess is worse than nothing; it is recorded with a number so that change has one.

### D10. Session ownership across a region

Unchanged in kind from the convolutional path, wider in degree. The chain acquires every session an
operation needs **before any of them runs**, holds each `SessionHandle` for the whole call, and drops
them when it returns. For Osaka that is three handles for one operation rather than one per pass.

Holding the handle *is* the guarantee: the session cache's "in use" test is the handle's own reference
count, so a model a run holds cannot be reclaimed by the idle sweep, by an explicit release, or by a
provider switch. This is the property the project context asks for — that Rust's ownership model turn
the reference's use-after-free into something unrepresentable — and the region loop needs nothing new
for it. The three handles travel into the blocking closure and back out of it exactly as the pass
sessions do today, so there is no window in which one could be reclaimed mid-region.

What is new is that the three may be built on **different providers**. The CPU fallback is per
session, so a machine where the transformer fails to build on CoreML but the VAE halves succeed will
run the region across two providers. That is correct and already reported —
`ProviderReport::new(provider, held.iter().map(SessionHandle::provider))` folds over every handle the
chain took — and it needs no change here. It is worth knowing it can happen, because a region split
across providers is a plausible explanation for a result that is slower than either provider alone.

## Risks / Trade-offs

**[The transformer does not load under `ort` 2.0.0-rc.13 even with the two optimizers disabled]** →
**Fired, and resolved — but not as expected.** Task 1.2 found the graph failing session creation with
the two optimizers disabled exactly as it fails with none disabled, and the cause is not the graph.
**ONNX Runtime 1.26 takes `optimization.disable_specified_optimizers` as a single transformer *name*,
not a comma-separated list**, so any two-name value matches no transformer and disables neither —
silently. Measured on the CPU provider against the published FP16 export:

```text
  (unset)                                            FAIL
  "SimplifiedLayerNormFusion"                        LOAD
  "ReshapeFusion,SimplifiedLayerNormFusion"          FAIL
  "SimplifiedLayerNormFusion,ReshapeFusion"          FAIL
  "ReshapeFusion, SimplifiedLayerNormFusion"         FAIL
  "SimplifiedLayerNormFusion,NotATransformerAtAll"   FAIL
  "ReshapeFusion"                                    FAIL
```

The first spelling was repeated last and gave the same answer, so this is not process carryover, and
neither node name in the failure appears anywhere in the published `.onnx`, so both are created at
load time. **No runtime bump is needed.** Three consequences, all folded into this change:

- **The profile names `SimplifiedLayerNormFusion` alone.** `ReshapeFusion` is not load-bearing for
  this graph, and naming it alongside would cost the model the one setting it cannot run without.
  This corrects the reference implementation rather than transcribing it, and it is the one figure in
  Osaka's profile measured here rather than inherited.
- **`EpProfile::disabled_optimizers` becomes `disabled_optimizer: Option<String>`**, so the value the
  runtime cannot honour is not writable. The `Vec` joined with commas was a trap for every model
  measured after this one: it would compile, run, report nothing and disable nothing. The
  `execution-providers` delta carries this as a requirement of its own.
- **The failure is CPU-specific.** On CoreML the graph opens either way, which follows from the node
  name — `InsertedPrecisionFreeCast_` belongs to the CPU provider's FP16 cast insertion. The setting
  is declared anyway, being a session setting that reaches every provider and the CPU being the
  fallback every machine can land on.

**[The scheduler step or the packing order is wrong and the result is merely bad rather than broken]**
→ This is the risk D1 is shaped around, and the mitigation is that the verification compares a decoded
region against its own input rather than eyeballing it. A velocity field decoded as an image scores
*worse* than the input it was given, which is a measurable property and not a matter of taste.

**[Slice 1 lands ~13 tasks of code that no public call reaches]** → Accepted, with D8's reasoning. The
exposure is bounded by slice 2 being the next change proposed, and by the arithmetic in this slice
being covered by tests that do not need a model.

**[The download is 6.8 GB and the measurements were made on hardware this change does not have]** →
The profile is transcribed rather than re-measured, and the design says so. What this change can
confirm locally is that the settings *reach* the provider and that the model loads and runs with them;
what it cannot confirm is the percentages. Those stay attributed to the reference in the comments.

**[Archive ordering against `add-upscale-tokyo-saitama`]** → **Resolved.** That change's spec delta
modifies two `execution-providers` requirements, and while it was complete but unarchived this one
carried its new requirement as `ADDED` rather than `MODIFIED` on either of them, so the two could
archive in either order without one silently dropping the other's scenarios. Tokyo archived first,
so the constraint has lifted: this delta now also carries a `MODIFIED` block on *A model's measured
settings include how the graph itself is run*, narrowing its "optimizers" to the singular the
runtime actually honours. That block is copied from the **post-Tokyo** main spec, which is the rule
any later change modifying those requirements still has to follow.

## Migration Plan

None. Nothing here changes an existing behaviour, a persisted format or a public signature: the image
cache tags are unchanged, `Upscale`'s serialized form is unchanged, and the one public type touched
(`GraphRole`, already exported) gains no variant. `Backend` is `pub(crate)`.

## Validated against

**Platforms:** written for macOS arm64, Windows and Linux; nothing in it is platform-specific. The
verification tasks run on macOS arm64, which is the machine this change has.

**Execution providers:** the CPU provider and CoreML are exercisable here. **CUDA and TensorRT are
untested by this change** — and the per-graph NHWC override in D3 is a CUDA-only setting, so its
*effect* is unverified even though its plumbing is tested. What CI proves is that the option map
carries the right value for each graph; what no machine here proves is the percentage behind it.

## Open Questions

- **Which precision to download first.** FP16 is the build the two disabled optimizers were diagnosed
  against, so it is the honest target for the load question; INT8 is half the download and exercises
  the messier CoreML path, since the reference records that ORT's CoreML EP has no `DequantizeLinear`
  builder and leaves that graph fragmented across ~36 partitions. This does not change the specs, the
  approach or the task breakdown — task 1 states FP16 and a task doing INT8 as well is additive.
