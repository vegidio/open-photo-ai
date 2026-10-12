## Context

See proposal.md — Why. What follows is the state of the code this design has to work with.

Three things are already true and shape everything below.

**Each family already publishes a `Params` type.** `UpscaleParams { scale: Option<Scale> }`,
`FaceRecoveryParams { faces }`, `DenoiseParams { strength }` and four siblings. They are typed values, one per family,
documented as the answer to "what one run takes that the operation's identity does not already carry". They are also
**dead**: `grep` finds no caller of `params()` outside `#[cfg(test)]`. `Upscale::pipeline` reaches past it to
`self.scale()`; every other family has no pipeline to reach with.

**The session registry does not key on an operation.** `sessions/mod.rs` declares
`type Key = (ArtifactId, ExecutionProvider)`, and its own doc comment explains the choice — two operations needing one
graph share a session, which is what the reference implementation gets wrong by keying on the operation. Nothing in
the crate keys a map on an `Operation` or on any family type.

That matters because eight hand-written `PartialEq` impls exist solely to serve that registry. Each drops the family's
per-run parameter so that dragging a slider does not read as a different model, and each says so in a doc comment.
`public_api.rs:199` pins the behaviour under the comment *"which is what a registry keyed on an operation depends
on"*. The registry it names has never existed.

**`ParameterEntry` is `{ name, min, max }`,** and `FamilyEntry.parameters` is family-wide with a doc comment stating
that a variant disagreeing with its siblings would let a control offer a value the others refuse. Face recovery
publishes `&[]` because a set of faces has no range — a known gap, recorded in a comment beside it.

## Goals / Non-Goals

**Goals:**

- One rule covering every model: a per-variant constructor, taking exactly what that variant takes, returning the
  carrier for the path it runs on. Twenty constructors, no exceptions, no family that reads differently.
- A parameter that belongs to one variant of a family and not its sibling, expressible in the type system and
  describable in the catalogue.
- `params()` consumed rather than declared, so a value the library publishes is a value the library uses.
- Caching driven by the parameters: the same image at the same fidelity is served, at a different fidelity is
  recomputed, and a Santorini run — which has no fidelity — cannot be made to miss by one.

**Non-Goals:**

- **Any inference.** No alignment, no similarity transform, no blend mask, no tile, no tensor. Athens and Santorini
  remain unrunnable when this slice lands, exactly as they are now. Slices 2 and 3 carry that, and the material they
  need is enumerated in the archived `add-models-3-faces` Non-Goals rather than re-searched.
- **Consuming `Analysis` or the new constructors in `gui` or `cli`.** Neither front end reaches these types yet.
- **Autopilot, crop, per-image face selection UI.** Unchanged and untouched.
- **Widening the run cache.** Its key derivation is already `identity | cache_tag | depth`, and every behaviour this
  slice needs falls out of what each family writes into its own tag.

## Slices

The face recovery series: making Athens and Santorini expressible, then running them.

| # | Slice | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-face-recovery-1-params` | `Fidelity`; the twenty per-variant constructors; the `Analysis` carrier; full-value operation equality with quantized faces; Athens' fidelity in the run cache tag; the catalogue's per-variant parameters and non-numeric kinds; `params()` consumed at the pipeline-construction seam | `add-models-3-faces`, `add-detection-newyork` (both done) | **this change** |
| 2 | `add-face-recovery-2-athens` | Athens' pipeline: the ArcFace landmark template and similarity transform, the 512px aligned tile, the feathered elliptical blend mask, the composite back into the frame, and the fidelity weight bound as a second input tensor | 1 | pending |
| 3 | `add-face-recovery-3-santorini` | Santorini's pipeline: the same alignment, tile, mask and blend through the family-tier code slice 2 promotes, over a graph that takes the image alone | 2 | pending |

Santorini is its own slice rather than folded into 2 because slice 2 carries the whole of the reference's hard-won
geometry, and because the promotion of that geometry from Athens' directory to family tier happens in the change that
introduces its second caller — which is slice 3. Splitting them keeps that promotion visible instead of pre-empted.

## Decisions

### D1. Fidelity is reachable only through Athens' constructor

`FaceRecovery` holds `fidelity: Option<Fidelity>` privately, and the field is set by
`FaceRecovery::athens(precision, faces, fidelity)`. `FaceRecovery::santorini(precision, faces)` sets it to `None` and
takes no such argument. `fidelity()` answers `Option<Fidelity>`.

*Alternatives considered.*

**One field, always present.** Simplest to build, and wrong in a way that is invisible until a user complains about
speed: two Santorini runs at different fidelities would report different cache tags for byte-identical output, so a
front end drawing one fidelity control for the family would silently halve the cache's hit rate on one of its two
variants. It also gives a caller a value to set that does nothing, which is the shape of mistake the typed `Params`
structs exist to prevent — the module comment on `FaceRecoveryParams` says so about the reference's `map[string]any`.

**Fidelity inside the variant — `FaceRecoveryVariant::Athens(FloatPrecision, Fidelity)`.** The strongest guarantee of
the three: handing Santorini a fidelity does not compile. Rejected because the variant is load-bearing elsewhere.
`artifact()` composes a filename from it, and `from_codename(codename, precision)` reconstructs one from what the
catalogue published — and the catalogue has no fidelity to supply. Both would need a second shape to route around the
field, to buy a guarantee the constructor split already provides.

The reference expresses the same distinction as `Fidelity: -1` on Santorini's row, a sentinel meaning "this graph has
no second input tensor". Go has nowhere better to put it. `Option` is the same fact without a magic number, and
`runInference`'s `if fidelity >= 0` branch becomes a match on `Option` at the point the tensor is built, in slice 2.

### D2. Every model gains a per-variant constructor, and it returns the carrier

Twenty of them: `Upscale::tokyo/kyoto/saitama/osaka`, `Denoise::stockholm/gothenburg/malmo`,
`Sharpen::moscow/stpetersburg/novgorod`, `Colorization::delhi/mumbai/jaipur`, `LightAdjustment::paris/lyon`,
`ColorBalance::rio/saopaulo`, `FaceRecovery::athens/santorini`, `Detection::newyork`.

Two properties fall out that the current `Type::new(Variant::Arm(precision), param)` spelling does not have.

**A precision a variant does not publish stops compiling.** `Upscale::osaka` takes `OsakaPrecision`; the other three
take `FloatPrecision`. Today the pairing is refused at construction; under this it cannot be written.

**The call site loses two layers.** `Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(fp16), scale))` becomes
`Upscale::kyoto(fp16, scale)`. The `Operation::Family(…)` wrapping disappears because the constructor returns the
carrier, not the family type:

```rust
let athens    = FaceRecovery::athens(FloatPrecision::Fp32, faces.clone(), Fidelity::new(0.5)?);
let santorini = FaceRecovery::santorini(FloatPrecision::Fp32, faces.clone());
let kyoto     = Upscale::kyoto(FloatPrecision::Fp16, Scale::new(4.0)?);

opai.process(&picture, &[athens, santorini, kyoto], None).await?;
```

*Alternatives considered.* `process` takes `&[Operation]`, and a Rust array is homogeneous, so a conversion has to
happen somewhere. `From` impls plus `.into()` at each element is the zero-cost baseline and was rejected only on
ergonomics. A declarative `operations![…]` macro hides the same `.into()` for the price of one export. Constructors on
`Operation` itself — `Operation::kyoto(…)` — would be shortest of all and was rejected on layout grounds: it puts
twenty model names into `operation.rs`, the cross-family file, so deleting a model would stop being its directory plus
the arms naming it. A `.chain(picture).then(a).then(b)` builder reads best and was rejected as a wider API change than
this slice needs.

The chosen form keeps every model name inside its own family's module. `Upscale::kyoto` lives in `upscale/mod.rs`,
which already names all four models in its `MODELS` table, so a family-tier file naming its family's models is
existing precedent rather than a new exception.

**The cost, stated plainly:** `FaceRecovery::athens` returning an `Operation` rather than a `FaceRecovery` is a
surprising signature, and Rust convention would expect `Self`. The typed value stays reachable by matching the carrier
— it is what `pipeline()` and `params()` are asked of internally. This is a deliberate trade of one surprise at the
definition for the absence of a wrapper at every call site.

### D3. A second carrier, `Analysis`, so detection follows the same rule

`Detection::newyork(precision)` cannot return `Operation` and keep what `execute` guarantees. `execute` is
`execute<O: DataOperation>(…) -> Result<Executed<O::Output>, InferenceError>`; `O::Output` is what makes a detection
run's result a `Faces` with nothing to unwrap, and the `compile_fail` doctest on `DataOperation` pins the other half —
an `Upscale` handed to `execute` does not compile. `Operation` can satisfy neither: one `Output` for seven image
families is either wrong or an enum, and an `Operation` carrying an `Upscale` would reach `execute` freely.

So there are two carriers, and the rule stays one sentence: **a model constructor returns the carrier for the path it
runs on.**

```
   Upscale::kyoto(..)      -> Operation  --> Opai::process(&picture, &[..], None)
   FaceRecovery::athens(..)-> Operation  -/
   Detection::newyork(..)  -> Analysis   --> Opai::execute(&picture, &analysis, None)
```

`execute` takes `&Analysis`. With one arm today it returns `Executed<Faces>` concretely, so nothing a caller writes
changes and nothing is unwrapped. Both compile-time properties survive: neither carrier can be handed to the other's
entry point.

*Alternatives considered.* **One carrier with a runtime refusal** — `execute` takes `&Operation` and errors for the
seven image families — is the most uniform and trades away exactly what `data-inference`'s "The result type is the
operation's own" requirement was written to state: that these are pairings which cannot be expressed rather than
requests which are accepted and then refused. **One carrier with an extractor** — `newyork.as_detection()?` — keeps
the constructors uniform at the price of a runtime unwrap in every caller.

**What a second data family costs.** `Analysis` gaining a second arm forces `execute`'s return to become an enum the
caller matches on, or forces the generic bound back. That is a real cost, deferred rather than avoided, and it is the
reason this decision is recorded rather than assumed.

**What it costs here: a third value, `Subject`, for the one place that names an operation without running it.** Both
paths report progress, and `InferenceProgress.operation` is an `Operation` — which detection is not, once it is
carried in `Analysis`. So the report's field becomes `subject: Subject`, where `Subject` is `Enhancement(Operation)`
or `Analysis(Analysis)` and forwards the five questions every family answers: `family`, `precision`, `display_name`,
`required_artifacts`, `cache_tag`. A front end that only labels a bar calls `display_name` and never matches.

Nothing a run reports changes. `execute` already had this problem and already solved it the other way, by wrapping
its detection back into `Operation::Detection(…)` purely to report it — a line that existed because the report could
name only one of the two shapes. `Subject` deletes the wrap rather than adding a case.

It lives in `inference/progress.rs`, beside the report, and not beside the carriers, because nothing dispatches on
it: it is reporting vocabulary, and neither entry point takes one. `From<Operation>` and `From<Analysis>` mean a
caller holding either hands it over unchanged, so `ChainProgress::operation` takes an `impl Into<Subject>`.

*Alternatives considered.* **Keeping `InferenceProgress.operation: Operation` and giving `Operation` a detection
arm** is the wrap made permanent, and it reopens what D3 exists to close: an `Operation` carrying a detection would
reach `process`. **A third report type for the analysis path** leaves a consumer wiring two callbacks and writing the
same label twice to learn what one run is doing. **Reporting a `String` display name alone** is enough for a bar and
nothing else — a consumer wanting the family, the precision or the cache tag would have to parse the label back.

### D4. Two operations are equal when every field matches

The eight hand-written `PartialEq`/`Hash` impls are deleted and replaced by derives. `Denoise` at strength 0.5 and at
strength 2.5 stop comparing equal; `Upscale` keeps comparing its scale, as it already does for the convolutional
variants and now does for Osaka too.

Nothing regresses, because nothing consumed the old behaviour. The registry those impls were written for keys on
`(ArtifactId, ExecutionProvider)` — see Context — so an 8x Kyoto and a 4x Kyoto already share `up_kyoto_4x_fp16`
however they compare, and a fidelity slider already cannot reload weights.

What it fixes is a trap pointing the other way. A front end asking *did the user change anything?* is asking for value
equality, and today gets `true` for two different requests. The library's answer to "the same pixels" stays
`cache_tag()`, which is unchanged in role; `==` becomes the answer to "the same request", and for the first time the
two agree.

*Alternatives considered.* **Leaving it as it is** costs nothing and keeps eight doc comments citing a registry that
does not exist, which is the kind of fiction that gets built on later. **Dropping `Eq`/`Hash` from operations
entirely** is the smallest honest surface — nothing uses them — and was rejected because value equality is something
a front end will plausibly want and cannot easily reconstruct.

### D5. A face quantizes on acceptance, to two decimals

`Face::new` rounds every coordinate it takes — the box's two corners and all five landmarks — to two decimal places,
and `Confidence::new` rounds the confidence, the way `Scale`, `Strength` and `Bias` already quantize on acceptance.
This is what keeps `Face` — and therefore `FaceRecovery` and `Operation` — `Eq` and `Hash` once D4 makes faces part
of value equality: a type containing raw `f32` can be neither.

**The quantization is at the face, not at the point.** `Point::new` carries whatever it is given, because a point is
built three times on the way out of the detector — once in the priors' normalised space, once in the detector's
square, and once in the source image's pixels — and only the last of those is a coordinate anyone would round.
Rounding the first to a hundredth would be rounding it to six pixels. `Face::new` is the one place a coordinate is
known to be in the image's own space, so it is the one place that rounds, and every point a `Face` holds has been
through it.

`Point` therefore still holds raw `f32`, and its `PartialEq`, `Eq` and `Hash` are hand-written over the canonical bit
pattern rather than derived: a derived `Eq` would demand `f32: Eq`, which does not hold. What writing them buys
beyond compiling is that `Eq`'s promise is true — the two spellings of zero are one coordinate, and a point carrying
`NaN` equals itself. `Rect`, `Face` and `Faces` derive all three on top of that, and `Confidence` stores an integer
count of quantization steps, so nothing above `Point` has to know.

Two decimals is not a new figure. `FaceRecovery::cache_tag` already spells every bounding box at two decimals, and
`face.rs` already carries a test asserting that a sub-hundredth difference is one selection. This makes that a
property of the value rather than of the string written from it, which is strictly stronger — the test stops depending
on the tag's formatter.

Approximately 0.01 px, against a detector whose FP16 and FP32 builds the reference found to differ only in sub-pixel
positions. Alignment is unaffected: the similarity transform slice 2 builds consumes landmarks at a scale where a
hundredth of a pixel is far below the noise the detector already carries.

**This reverses a decision made deliberately.** `face.rs`'s header argues that none of these values is rounded,
rescaled or quantized on acceptance, because sub-pixel positions are the whole of what distinguishes two detection
runs. That paragraph is rewritten as part of this change rather than left to contradict the code — the reasoning it
records is sound about detection and was written before anything compared an operation by value.

*Alternatives considered.* **Quantizing only inside `Eq`/`Hash`,** leaving stored values exact, keeps both traits and
changes no data, and produces `a == b` while `a.faces() != b.faces()` — a trap of a different shape, and a worse one,
because it is invisible. **Accepting the loss of `Eq`/`Hash`** delivers D4 with no change to `Face` at all and was the
narrower option; it was rejected in favour of keeping `Operation` usable as a map key, which costs precision no
consumer of this library can observe.

### D6. `params()` is consumed at the pipeline-construction seam, not at `run`

Each family's `pipeline()` is built from its `Params` value. `Upscale::pipeline` hands `self.params()` to
`PassSequence::new` and `Diffusion::new` instead of reaching past it to `self.scale()`; face recovery's pipeline, when
slice 2 writes it, is constructed from `FaceRecoveryParams { faces, fidelity }`.

**`UpscaleParams.scale` stops being an `Option` as part of this.** It was `Option<Scale>` — `None` for a
convolutional variant, whose scale selected its weights and was therefore already in the operation, `Some` for Osaka,
where one set of weights serves every scale. That distinction is about the *identity*, and D4 removes it: the
identity now carries every field, on every variant. What is left in a `Params` is what a **run** takes, and every
upscale run takes a scale — the convolutional pipeline to pick its passes and correct the result, the diffusion
pipeline because its weights do not encode one. Leaving the `Option` would make `pipeline()` unwrap a value that is
always present, on the seam this decision exists to make load-bearing, which is the shape of fiction D4 deletes
elsewhere. This is a breaking change to a public type; `UpscaleParams` is re-exported, and a caller matching on the
`Option` stops compiling rather than silently reading a different value.

*Alternative considered:* carrying params into `ImagePipeline::run` instead. `Shared<B> = Arc<dyn ImagePipeline<B>>`
is a trait object with no per-family type available at that seam, so params would have to travel as a cross-family
`OperationParams` enum that each pipeline matches on — reintroducing exactly the "wrong parameter for this family"
runtime failure the typed per-family structs were built to make unwritable. The only thing it would buy is running one
built pipeline at several parameter values, and nothing does that: a pipeline is constructed per operation per run,
and what is expensive is the session, which is cached separately and keyed on the artifact.

### D7. The catalogue publishes parameters per variant, and describes more than a range

Two edits to `catalogue.rs`, both forced by Athens.

**`parameters` moves from `FamilyEntry` to `VariantEntry`.** Fidelity belongs to one variant of a family, which the
current shape states is impossible. The move is also the more truthful home independently of Athens: a front end draws
controls for a selected variant, never for a family, so it reads parameters where it reads the variant. Seven families
repeat one entry across their variants, which is duplication in the published JSON and not in the source — the entries
are `&'static` borrows of the same constants.

**`ParameterEntry` grows a kind.**

```rust
pub enum ParameterKind {
    Range { min: f64, max: f64 },   // strength, bias, scale, fidelity
    Faces,                          // supplied by a detection run
}
```

Face recovery publishes `Faces` instead of `&[]`. Today its entry and colorization's are byte-identical, and only
colorization's is true: colorization takes nothing, while face recovery **cannot be constructed** without a set of
faces. A front end has to know from somewhere else that choosing Athens means running a detection first — which is the
one thing the catalogue exists to prevent. The comment beside the empty list already records this as a known gap
rather than an oversight.

*Alternative considered:* keeping `parameters` on the family for the shared ones and adding a second list on the
variant for the extra ones. Two places to look for one answer, and a front end that read only one would be wrong about
Athens or about every other family.

### D8. The round trip from a catalogue row back to an operation

`from_codename(codename, precision) -> Option<Variant>` is how a front end turns what it read from the catalogue back
into something runnable, restating no model vocabulary of its own. Under D2 a variant is no longer sufficient: the
constructor is per-variant, so a caller holding `FaceRecoveryVariant::Athens(Fp32)` still needs a fidelity, and one
holding `Upscale`'s Osaka arm needs an `OsakaPrecision`.

The round trip is preserved by keeping `from_codename` exactly as it is and matching on what it returns. The caller's
branch is over an enum the compiler checks, and the value it needs for each arm is the one the catalogue told it that
variant takes — which is now per-variant, per D7. The two decisions are the same decision seen from the two ends.

### D9. Crates, dependencies and `rust-sak`

Nothing is added. No `rust-sak` feature is newly used, and none is missing: this slice adds bounded parameter types,
enum shape and constructors, all of which are this project's own domain. `Fidelity` is local for the same reason
`Strength`, `Bias` and `Scale` are — a range with a name and a quantization step, meaningful only here.

`gui` and `cli` are untouched. Neither reaches any of these types today, so there is no front end to keep logic out
of; when they do, what they consume is the catalogue, which is why D7 is in this slice rather than deferred.

### D10. Platforms, providers, sessions and lifetimes

**No session is built and no provider is exercised.** Every value here is plain data describing what to run, with no
native handle, no `Drop` and no reference to anything loaded — the same property the model-naming series established
and the reason a front end can hold and persist one. `Faces` stays `Arc<[Face]>`: shared ownership of immutable data,
so cloning an operation is a refcount bump.

Nothing varies by operating system, architecture or execution provider: no artifact name, label, precision or bound is
platform-dependent, and the quantization in D5 is decimal rounding with no platform-specific behaviour. CI's existing
matrix covers it, and **no execution provider was validated, because this change builds nothing to validate one
against.** The per-graph `EPProfile` material for Athens and Santorini — which the reference measures per graph *and*
per precision, and warns explicitly against generalising between them — stays deferred to slices 2 and 3, where a
session first exists.

## Risks / Trade-offs

- **The twenty constructors are a wide breaking change across a public API, and a mistranscribed codename is a
  constructor that names an artifact which does not exist** → Each constructor is one line delegating to the existing
  `Type::new(Variant::Arm(precision), …)`, so the codename it resolves to is the variant arm's, not a new literal.
  The existing per-family tests that assert every variant names its published artifact are driven through the new
  constructors, so a wrong arm fails them.
- **`FaceRecovery::athens` returning `Operation` rather than `Self` violates a Rust reader's expectation** → Accepted,
  and D2 records it as the price of the call site. Mitigated by its being uniform: all twenty read the same way, so it
  is a convention to learn once rather than an exception to remember.
- **Quantizing faces discards detector precision permanently, and a future need for sub-pixel coordinates would be a
  second reversal** → The value discarded is below what the detector distinguishes between its own FP16 and FP32
  builds, and the cache tag already treated it as noise. If a later family genuinely needs sub-pixel landmarks, it
  needs them from the detector's own output rather than from a persisted operation, which is a different path.
- **Inverting operation equality silently changes behaviour for any consumer relying on the old semantics** → There
  are none inside the crate, and `gui` and `cli` construct no operations yet. `public_api.rs:199` is the only place
  the old behaviour is pinned, and inverting that assertion is a task rather than a discovery.
- **`Analysis` is a carrier with one arm, which reads as speculative structure** → It is not added for a second data
  family; it is what lets detection's constructor follow D2's rule at all. The alternative was an inconsistent
  eighth family or a runtime refusal on `execute`, both recorded in D3.
- **Two families named, parameterised, catalogued and still unrunnable could read as complete** → The same risk the
  model-naming series carried and the same mitigation: the Non-Goals above name every deferred piece, and the Slices
  table says which change carries it.

## Migration Plan

Not applicable in the deployment sense — nothing is persisted that this change invalidates. Two notes for whoever
implements it:

- **Cache tags are unchanged for seven families and for Santorini.** Athens' tag gains a `-w<value>` segment, so any
  Athens entry written before this change misses once and is swept at the 24-hour TTL. Nothing else on a user's disk
  is affected. No Athens run has ever been executed, so in practice there is nothing to invalidate.
- **The serialized form of an operation is unchanged.** `Face` gains no field, and quantization applies on
  construction, so a persisted selection deserializes to the same value it did before — now rounded, which a
  round-trip test asserts is stable rather than drifting on each pass.

## Open Questions

- **What a front end should offer as the default fidelity, and how the control is labelled.** `1.0` is the
  reference's value and the parity default, and it is the *least* aggressive setting — maximum fidelity to the
  original face, minimum restoration. Whether a user-facing control should present it as "fidelity" at `1.0` or
  invert it into "strength" at `0.0` is a front-end question, answerable when `gui` first draws the control, and it
  changes nothing here: the library publishes the range and the name the error message uses.
