## Why

Osaka is the last of the four upscale variants still refused. Tokyo, Kyoto and Saitama run;
`inference::chain::plan_one` turns Osaka away with *"the diffusion upscaler needs a pipeline of its
own"*, which is accurate — it is a VAE encoder, a one-step diffusion transformer and a VAE decoder
run as three stages of a single pass over the image at its **target** size, rather than a sequence
of native scaling passes. Every piece of vocabulary it needs is already in place: the variant row,
`GraphRole`, `Resolution::Graphs`, the FP16 pins on the VAE halves, `Normalisation::Signed` sitting
under an `expect(dead_code)` waiting for its first caller. What is missing is the pipeline.

That pipeline does not fit in fifteen tasks, so it is two slices. **This is slice 1**, and it is cut
by risk rather than by tidiness. Three things in this port can be wrong without announcing it: the
transformer may not load at all under the pinned ONNX Runtime 1.26, the scheduler step may be read
wrongly (the graph's output is named `denoised_latent` but is the flow-matching velocity), and the
33-channel packing order is not discoverable from the graph — all 33 channels go through a single
projection, so a wrong order returns a worse image rather than an error. None of the three is
testable without the real model. The pieces slice 2 carries — the tile grid, the blending
accumulator, the wavelet colour fix — are pure arithmetic that cannot be silently wrong.

So slice 1 ends where those three unknowns are answered: one 960x960 region, encoded, restored and
decoded, verified against the real model.

## What Changes

- **Verify the three published exports before writing anything against them.** Confirm the input and
  output names and shapes, the 33-channel transformer input, that the timestep really is folded into
  the graph as a constant rather than left as an input, and that the transformer fails to load under
  the pinned runtime **without** the two disabled optimizers and loads with them. The precedent is
  `add-upscale-tokyo-saitama` tasks 1.1-1.2, which found one of the reference's own figures wrong.

  **It did.** Every shape and constant is what the exports declare. The load behaviour is not: the graph fails on the
  CPU provider with the two named *together*, because ONNX Runtime 1.26 takes that setting as a single transformer
  name rather than as a list. `SimplifiedLayerNormFusion` alone is what loads it, `ReshapeFusion` is not load-bearing
  at all, and the failure is CPU-specific. See design.md's Risks section for the measured table.
- **Osaka's measured execution-provider settings**, transcribed from the reference: sequential
  execution, the memory planner off, `SimplifiedLayerNormFusion` disabled, CoreML restricted to the
  CPU and GPU, and TensorRT's builder optimization level dropped from 5 to 3. One of those is not
  transcribed but measured here, and it corrects the reference — see above.
- **`EpProfile` narrows its disabled-optimizer field to one optional name**, because a list of two is
  a value the pinned runtime cannot honour and silently ignores. It was a trap for every model
  measured after this one, and the `execution-providers` delta carries it as a requirement.
- **A per-graph override.** Osaka is the first model whose graphs do not share one provider
  configuration: the two VAE halves want the CUDA NHWC layout (-8.0% and -9.4%) and the transformer
  does not (+1.3%, and it contains no convolution at all for an NHWC kernel to be faster at).
- **Shaped inference.** `inference::session::run_tile` derives its tensor shape from the buffer —
  `sqrt(input.len() / 3)`, hardcoded to three channels and square. Osaka's three graphs run
  `[1,3,960,960]`, `[1,33,120,120]` and `[1,16,120,120]`, so the seam gains a shaped sibling.
- **`Planned` splits** into the two contracts `Resolution` already distinguishes: a pass sequence
  with one shared profile, or a role-addressed graph set where each graph carries its own.
- **The region itself**: the 33-channel latent packing, the deterministic per-region noise field, the
  scheduler step, and the three-graph call that joins them.
- **The refusal stays up.** Osaka remains refused through the public API for the whole of this slice;
  slice 2 lifts it. Slice 1's deliverable is verified by a real-model test rather than by public
  reachability — see design.md D8 for why that is the cut and what it costs.

### Deliberate divergence from the reference's behaviour

**The noise field will differ, so the same image at the same settings will not be pixel-identical
between the Go application and this one.** Go seeds with `math/rand/v2.NewChaCha8`, which is not
plain ChaCha8 but Go's own ChaCha8Rand construction with its own reseeding schedule; no Rust crate
reproduces that stream. The property the seed exists for is preserved in full — the reference's own
comment says it is there so the image cache cannot memoize a result the user is unable to reproduce,
and so an A/B comparison of settings means something, both of which are about determinism *within*
one implementation. Bit-parity with Go is not among them, and nothing else in this project claims it.

**No memory-pressure warning.** The reference logs before a run it expects to exhaust the pool,
reading a per-pool budget from `internal.Registry.Available`. This project has no memory budget —
`sessions/mod.rs` already records that as a deferral — so there is nothing to read and nothing
honest to warn from. Recorded as a parity gap rather than half-built.

## Capabilities

### New Capabilities

None. Everything here extends a capability that already exists.

### Modified Capabilities

- `execution-providers`: a model's measured settings may differ **between the graphs of one model**,
  rather than being one declaration per model and precision. Plus Osaka's own declaration, and the
  narrowing of the disabled optimizer to a single name — which also reworks *A model's measured
  settings include how the graph itself is run*, whose "optimizers" the pinned runtime never honoured
  in the plural.
- `models`: Osaka's resolved graph set carries the provider settings measured for each graph
  alongside the artifact serving it, so a graph cannot be opened under another graph's configuration.
- `inference-tiling`: a model may be fed a tensor whose channel count and spatial shape are declared
  rather than inferred from the buffer, which is what lets a latent-space graph run at all.

## Impact

**Crates**: `opai` only. Nothing here is user-visible yet — the refusal stays up — so neither front
end changes, and no i18n key or Tauri command is added.

**rust-sak**: checked, and it has **nothing** for the one piece of infrastructure this slice needs.
The per-region noise field wants a deterministic, seedable pseudo-random generator; `crypto` is
SHA-256 and XXH3-64 hashing, which is not a generator, and no other module is close. This is not a
gap worth filling in `rust-sak` either — a PRNG is not a first-party concern and the ecosystem's
answer is settled. Slice 1 therefore adds `rand_chacha`, which is the crate the Rust Rand Book names
for exactly this case: reproducibility needs a *named* algorithm rather than `StdRng`, which carries
no cross-version stability guarantee. `rand` itself is deliberately **not** added — see design.md D5.

**Files**: `inference/session.rs` and `inference/chain.rs` (the seam and the `Planned` split), a new
`inference/osaka/` module for the region, `models/upscale/variant.rs` and `resolve.rs` (the profile
and its per-graph override), `providers/profile.rs` (narrowing the disabled-optimizer field), and
`sessions/build.rs` and `providers/options.rs` carrying that narrowing through to the session.

**Verification cost**: no Osaka weights are on the development machine — neither the Rust app
directory nor the Go one has them. The verification tasks need a download of ~3.4 GB (INT8) or
~6.8 GB (FP16). The FP16 build is the one whose graph the disabled optimizer was diagnosed against,
so it is the honest target. **Paid**: the three FP16 artifacts are 7.2 GB as published, and the live
checks install into a directory they keep rather than a temporary one so the transfer happens once.

**Task count**: 13, estimated before writing this. The series is two slices; slice 2
(`add-upscale-osaka-2-pipeline`) is ~12 and is not proposed until this one archives.
