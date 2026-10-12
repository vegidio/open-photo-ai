## Slices

This refactor is three changes. Slice 1 establishes the seam and moves no files; slices 2 and 3 are
then file moves with no dispatch edits, because after slice 1 nothing outside a model's own code names
a model.

| # | Change | Delivers | Depends on | State |
| --- | --- | --- | --- | --- |
| 1 | `refactor-model-layout-1-seam` | The `Pipeline<B>` trait, both impls in place, `chain.rs` made model-blind, the `Operation` → `Pipeline` seams in `models/`, and the module-layout doctrine written into `config.yaml`. No files move. | — | **current** |
| 2 | `refactor-model-layout-2-osaka` | `inference/osaka/` disappears. `canvas.rs` is promoted to `inference/`; `mod.rs`, `latent.rs`, `region.rs`, `noise.rs`, `colorfix.rs` and `live.rs` move to `models/upscale/osaka/`. The multi-graph vocabulary (`GraphRole`, `Graph`, `DiffusionVariant`, `GraphSet`, `ResolvedGraph`, `UnknownRole`) moves to `models/upscale/osaka/graph.rs`, `OsakaPrecision` to `models/upscale/osaka/precision.rs`, and the `OSAKA` row, its profile arm and `BROKEN_OPTIMIZERS` into `osaka/mod.rs`. The `models/` and `inference/` module docs are rewritten to the new doctrine. | 1 | pending |
| 3 | `refactor-model-layout-3-conv` | `inference/upscale.rs` becomes `models/upscale/passes.rs`; `ConvVariant`, `ScaleBucket` and `FOUR_ONLY` become `models/upscale/conv.rs`; the `TOKYO`, `KYOTO` and `SAITAMA` rows with their profiles, measurement prose and tests move into `models/upscale/{tokyo,kyoto,saitama}/`. The six exhaustive matches over `UpscaleVariant` collapse into one `model()`. `catalogue.rs` is repointed. One directory per model, done. | 2 | pending |

Slice 1 is deliberately the one that carries the `config.yaml` doctrine rewrite, although the layout it
describes does not exist until slice 3. The alternative — document the rule once it is true — would
have slices 2 and 3 establishing a convention retroactively, and would leave the two moves without a
written standard to be reviewed against.

## Context

See `proposal.md` — Why. What matters for the approach:

- `inference/chain.rs` is the whole body of `Opai::process`. It already carries two hand-rolled
  vtables — `Planned` (a two-arm enum of what an operation will run) and `Work` (a two-arm enum of
  what its blocking half will run) — plus `plan_one`, `plan_graphs` and a `match &work` that calls
  `run_passes` or `osaka::restore_image` by name. Those five sites are the coupling; the enums are
  already the trait, written by hand.
- `Backend` is generic with associated `Session` and `Error` types. Production instantiates it at
  `ort::session::Session` and `ort::Error`; the test suite instantiates it at a counting fake. That is
  what keeps the refusals, the ordering, the three progress weightings, the depth resolution, the
  identity composition and cancellation as checked properties on every CI platform with no runtime,
  no GPU, no model file and no network. **Nothing in this slice may cost that.**
- `Resolution` already distinguishes the two contracts — `Passes` for the convolutional variants,
  `Graphs` for the diffusion one — and `Upscale::resolve()` already answers it. The contract seam
  exists; only the pipeline seam is missing.
- Every profile a session is opened under travels attached to the artifact it was measured for
  (`Planned::sessions` pairs them). That pairing is load-bearing: a graph opened under another
  graph's configuration produces a working session and a wrong or slower result that nothing
  downstream can detect. Any new trait must keep the artifact and its profile in one value.

## Goals / Non-Goals

**Goals:**

- `chain.rs` compiles with no `use` of any model or pipeline module, and no identifier naming a model.
- A new model is reachable through one arm in one match; a new *contract* through one arm in a second
  match. Neither match is in `inference/`.
- The fake-backend test suite continues to drive both contracts with no ONNX Runtime present.
- The doctrine governing all three tiers, and the promotion rule that keeps them honest, is written
  where every future change reads it.

**Non-Goals:**

- Moving any file. Slices 2 and 3 do that.
- Changing `Backend`. Its `run_tile` / `run_graph` split is a second, smaller leak — the backend
  knows about contracts — and collapsing `run_tile` into a `run_graph` convenience is a separate
  cleanup with its own risk to the tiled driver's shape-recovery guarantee. Deferred, deliberately,
  and not to any of the three slices here.
- Touching the seven unwritten families. Their refusal moves from `chain.rs` to `models/`; it is the
  same refusal with the same message and the same exhaustive match forcing an answer.
- Any change to the measured EP profiles, or to the prose recording how they were measured.

## Decisions

### D1 — `Pipeline` is generic over the backend, not over the model

```rust
pub(crate) trait Pipeline<B: Backend>: Send + Sync {
    fn required(&self) -> &[ArtifactId];
    fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)>;
    fn stages(&self) -> usize;
    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError>;
}
```

`B` is a parameter of the trait rather than of its methods, so `dyn Pipeline<B>` is object-safe and
`chain.rs` can hold an `Arc<dyn Pipeline<B>>` per planned operation, named `Shared<B>`. Putting `B` on
`run` instead would make the trait non-object-safe and force the caller back to an enum — which is
what we are removing.

`Arc` rather than `Box`, and `Send + Sync` rather than `Send`, for one mechanical reason: the run is
handed to a blocking thread, which takes a `'static` closure, while the plan itself stays on the
runtime and is indexed by a loop that can walk backwards over a prefix it had skipped. Borrowing the
pipeline into the closure does not typecheck and moving it out of the plan loses it. A clone per
*operation* is the same idiom the progress reporter beside it already uses. Both implementations are
plain data — a pass sequence and its profile, or a graph set and the positions its roles were acquired
at — so neither pays anything for `Sync`.

`sessions()` keeps the existing `(&ArtifactId, &EpProfile)` pairing verbatim, which is what preserves
the guarantee in Context: the acquisition loop cannot open one graph under another's configuration
because it never sees a profile that is not already attached to its artifact.

Rejected: erasing `Backend` to a `dyn` object so `Pipeline` could be non-generic. It would require
`Session` to become `Box<dyn Any>`, discarding the typed `SessionHandle<B::Session>` that makes the
session cache's reference-count guarantee a compile-time one — the single sharpest edge this project
inherited from the Go original.

### D2 — Two seams, one per axis of variation

Model-level and contract-level dispatch are separated, because they vary independently and a single
match conflating them grows on both axes at once:

```
models/upscale/variant.rs      SEAM 1 — one arm per MODEL
    UpscaleVariant::model(self) -> &'static dyn UpscaleModel
        Tokyo(_) | Kyoto(_) | Saitama(_) | Osaka(_)

models/upscale/mod.rs          SEAM 2 — one arm per CONTRACT
    Resolution::Passes(..) -> passes::pipeline(..)
    Resolution::Graphs(..) -> osaka::pipeline(..)
```

`UpscaleModel` is deliberately **not** generic: it answers only the data questions — codename, label,
profile, resolve — so it is object-safe and usable as a `&'static dyn` in const position. The
backend-generic half is reached through `Resolution`, which already names the contract. A fifth
convolutional model therefore touches seam 1 only; seam 2 grows only if a genuinely new contract
arrives.

Rejected: a registry of `&[&dyn UpscaleModel]` with no enum. It makes removal a one-line edit, but a
model added without a registry entry compiles clean and is silently absent, and `(codename,
precision)` pairings that are unrepresentable today — `osaka` at FP32 has no construction path — become
runtime refusals again. The exhaustiveness is the thing the existing code argues hardest for, in
`variant.rs`'s own header.

### D3 — `Planned` and `Work` are deleted rather than kept beside the trait

`Planned` becomes the boxed trait object plus the operation it came from; `Work` disappears entirely,
its role-position mapping (`GraphRole::{Encoder, Transformer, Decoder}` → indices into the acquired
sessions) moving inside Osaka's own impl, which is the only thing that ever needed it. `plan_graphs`
goes with it. `plan()` keeps its shape and its guarantee: the whole chain is judged before anything is
installed, so a chain whose second operation cannot be served transfers nothing on behalf of its first.

### D4 — Ownership and lifetime are unchanged, and the trait is shaped to keep them that way

`run()` takes `&[SessionHandle<B::Session>]` by shared reference. The chain acquires every handle for
an operation before any of it runs, moves them into the blocking closure and back out, and holds every
one until the whole chain returns — which is what makes "a session a run holds cannot be reclaimed by
the idle sweep, by an explicit release, or by a provider switch" a property of the reference count
rather than of a lock. Passing handles by reference rather than by value keeps that ledger in
`chain.rs`, where it is today: a pipeline borrows sessions for the duration of `run` and cannot retain,
drop or reorder them.

Three handles for one operation (Osaka) versus one per pass (convolutional) is a difference in degree
that the slice `&[..]` already expresses. No pipeline may acquire a session itself; `Backend::acquire`
stays on the chain side, which is what keeps install progress reported once per operation rather than
once per pass.

### D5 — The doctrine goes in `config.yaml`, not in a spec

All twelve existing specs are behavioural — Requirements with runnable Scenarios. A module layout has
no observable behaviour to write a Scenario against, and this project's own `specs` rule forbids
requirements written against internal structure. `config.yaml`'s `context:` block is the mechanism
that already carries the project's non-negotiables (`rust-sak` comes first, the 15-task slicing rule)
and it is injected into every future change's proposal, design and tasks — so whoever proposes the
denoise family reads it without going looking. The detailed reasoning is mirrored in the `models/` and
`inference/` module docs, where someone opening the code meets it.

The bullet currently in `config.yaml` under Conventions — "a model family is a DIRECTORY, with
`mod.rs` for the operation and `variant.rs` for what can be named…" — is replaced by two sections:

```
# Module layout

Three tiers. The test for which one a file belongs to is "who else could use it":

  ONE MODEL     -> models/<family>/<model>/       e.g. models/upscale/osaka/
                   Its row, its measured EP profile and the prose behind it, its
                   pipeline, the vocabulary only its contract needs, its tests.
                   Deleting a model is deleting its directory plus one enum arm
                   and one model() arm. Nothing else.

  ONE FAMILY    -> a FILE under models/<family>/  e.g. models/upscale/passes.rs
                   Shared by two or more of that family's models, by nothing outside.

  EVERY FAMILY  -> models/ for vocabulary, inference/ for execution primitives.
                   Never one family's, never one model's.

A model's execution code lives WITH THE MODEL, not in inference/. inference/ holds only
what no model owns: the tile grid, padding, tensor conversion, the two blending
strategies, the session seam and the chain driver. The chain driver reaches a model
only through Pipeline and names no model.

A family exposes its models through exactly two seams:
  1. one match from the variant enum to &'static dyn <Family>Model   (one arm per MODEL)
  2. one match from the resolved contract to a Pipeline              (one arm per CONTRACT)
A new model is a directory plus one arm in seam 1. A new contract within a family is one
arm in seam 2. Neither ever reaches chain.rs.

Tests live in the directory of the thing they cover.

# A file's tier is decided by its callers, and is re-decided every time one is added

The three tiers describe who uses a file TODAY, not who wrote it. When a second caller
appears, the file MOVES:

   a MODEL-tier file a second model in the same family needs      -> family tier
   a FAMILY-tier file a second family needs                       -> cross-family tier
   a FAMILY-tier file left with one caller (a model was removed)  -> back down into it

Two things are never the answer to a second caller:

  * IMPORTING ACROSS A MODEL BOUNDARY. A `use` that reaches from one model's directory
    into another's is a promotion that has not happened yet. It is a bug, not a shortcut.
  * COPYING. inference/mod.rs already records the reference implementation's own
    conclusion on this: written inside one pipeline it would be rewritten inside the
    next, and the two would disagree.

WHEN. The promotion happens in the change that introduces the second caller, and BEFORE
that caller is written, so the new model is built against the shared location from its
first line. Where the move will not fit that change's 15-task budget it becomes its own
slice ordered AHEAD of the one that needs it - never a follow-up afterwards, which is how
a "temporary" copy gets made permanent.

KNOWN CANDIDATES TODAY, parked at model tier because each has exactly one caller and is
expected to gain a second:
  models/upscale/osaka/colorfix.rs   wavelet colour fix for one-step diffusion drift
  models/upscale/osaka/noise.rs      deterministic seeded per-region noise
Both are already written model-agnostically. The second diffusion model in any family
promotes them; it does not reimplement them.
```

The promotion rule is the half that makes the rest safe. Without it, `colorfix.rs` and `noise.rs`
sitting under `osaka/` reads as a judgement that they are Osaka-specific, which is wrong and would get
them reimplemented by the next diffusion model rather than promoted.

## Division across crates

`opai` only. `gui` and `cli` reach inference through `Opai::process`, `Operation` and the catalogue,
none of which change shape; `perf` likewise uses only the public API. Nothing moves toward a front end
and nothing that a front end would need is introduced.

## Dependencies

**`rust-sak`:** no feature is added or dropped. This slice writes no infrastructure — nothing touching
hashing, HTTP, retries, downloads, extraction, temp files, config directories, image codecs, caching,
telemetry or hardware probes — so there is nothing for `rust-sak` to cover and no gap to judge. Its
existing use inside the touched files (`crypto::xxh3_string` behind `identity_after`, `memo` behind
`RunCache`) is carried through unchanged.

**Other crates:** none introduced. The trait is written against types this crate already owns.

## Platforms and execution providers

The refactor is validated by the existing fake-backend suite, which is arithmetic and runs on macOS,
Windows and Linux with no ONNX Runtime, no GPU, no model file and no network — so every property it
covers is checked on every platform. Beyond it, the live suites (`inference/live.rs`,
`inference/osaka/live.rs`) exercise the real graphs; those are macOS/Apple Silicon on CoreML and CPU on
the hardware available here. **CUDA and TensorRT are untested for this slice**, as they are for the
changes that introduced the code being refactored. That is acceptable because no provider-selection
code, no session-building code and no EP profile is touched: the profiles pass through the new trait as
the values they already are, and a test asserts the pairing is preserved.

## Risks / Trade-offs

- **A profile is separated from its artifact while reshaping `Planned::sessions` into a trait method.**
  → The method keeps the `(&ArtifactId, &EpProfile)` tuple exactly as it is today, and a test asserts
  an Osaka plan hands back each graph's own profile — specifically that the transformer's
  `cuda_prefer_nhwc` is `false` while the two VAE halves' is `true`, which is the one per-graph
  override in the project and therefore the case that would break silently.
- **The fake-backend suite is weakened while being rewritten against the trait.** → No test is deleted
  in this slice. Each existing assertion is repointed, and the task list calls for the test count
  before and after to be compared, not just for the suite to pass.
- **Holding a pipeline behind an `Arc` per operation adds an allocation, a refcount clone and a
  virtual call.** → One of each per *operation*, against a tile loop that runs thousands of times per
  operation and a session build that costs seconds. Unmeasurable. None of it is in the tile loop,
  which stays monomorphised.
- **`chain.rs` is the most heavily documented file in the crate (3,208 lines, most of it reasoning).**
  Deleting `Planned`, `Work` and `plan_graphs` deletes their doc comments, several of which record
  *why* the two contracts are kept apart — reasoning that still applies to the trait. → The task list
  requires that reasoning to be carried onto the trait and its two impls rather than dropped, and the
  module header's "what is deliberately not here" section to be updated rather than left describing a
  structure that no longer exists.
- **`Operation::profile` loses its last production caller and is left alive for the suite alone.** A
  session is now built from what `Pipeline::sessions` hands back — pairs of an artifact and the
  settings measured *for that artifact* — so nothing outside `models/` looks a profile up beside the
  thing it belongs to any more. What the method still answers is the cross-family assertion the suite
  holds all eight families to: that each variant declares a profile, and that only the measured ones
  declare anything but the default. → Kept behind `#[cfg_attr(not(test), expect(dead_code, ...))]`
  rather than `allow`, so that `expect` fails the build the day it gains a caller again and it cannot
  rot unnoticed. **Left knowingly open for slice 3**, which collapses the six exhaustive matches over
  `UpscaleVariant` into one `model()` and is the change that should settle whether the assertion is
  better expressed as a test helper than as a production method kept alive for the suite.
- **The doctrine describes a layout that will not exist for two more slices.** → It is written as the
  architecture the series establishes, and the Slices table above is repeated in every slice's
  design.md, so a reader who finds the rule ahead of the code finds the plan beside it.

## Open Questions

None. The three forks this design turns on — where the diffusion helpers land, how models are
dispatched, and where the multi-graph vocabulary belongs — were settled before the change was
scaffolded, and each is recorded above with the alternative that was rejected.
