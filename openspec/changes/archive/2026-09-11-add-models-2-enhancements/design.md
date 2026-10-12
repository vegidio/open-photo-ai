## Context

See `proposal.md` - Why. What matters for the approach is that the vocabulary already exists.

Slice 1 landed `Precision`, `ConvPrecision`/`OsakaPrecision`, `Family` with all eight prefixes,
`ArtifactId`, `RangeError`, `Scale`, the catalogue types, and the `Upscale` operation with its two
contracts. `Family` already declares `Denoise`, `Sharpen`, `LightAdjustment`, `ColorBalance` and
`Colorization` with their prefixes, and `RangeError` was already written with a `parameter: &'static
str` field so a second ranged type could reuse it. This slice is the extension that vocabulary was
shaped for, not a rework of it.

The reference implementation (`../open-photo-ai/models/`) is the behavioural source of truth for every
id, label and range. Three of its files were read against rather than assumed: `models/ids_test.go`
pins all thirteen artifact ids as literals, each family's `variant.go` carries the `Op` shape and its
`Params`/`CacheKey` contributions, and the `Options*` React components carry the bounds. The bounds are
the one place the Go and TypeScript halves disagree about who is responsible: the components clamp,
the Go code never does.

## Goals / Non-Goals

**Goals:**

- Thirteen model names that cannot be constructed in a state the published artifacts do not cover.
- Two ranged parameter types with the same guarantees `Scale` already gives, so that the thing the
  reference leaves to a React component becomes a library fact.
- A catalogue complete enough that a front end needs no model vocabulary of its own for any of the six
  families it can now offer.

**Non-Goals:**

- Any inference, tiling, blending or session building. `BlendWithIntensity`, the NAFNet divergence
  guard, light adjustment's canvas geometry and colour balance's `MixedSpec` channel layout are **not**
  ported here - they belong with the pipeline that runs the graph. Nothing about them is lost; they
  are read when inference is designed.
- Downloading model artifacts. This slice names them.
- The model registry and memory budgets. No session exists yet - see *Ownership and lifetimes*.
- Face Recovery and Detection, and any GUI or CLI work.

## Slices

This series is the model scaffold for the six families being scaffolded now, and nothing else. It was
estimated at roughly 24 tasks, over the 15 limit, so it is split in two.

| # | Slice | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-models-1-upscale` | Shared model vocabulary (precision, family prefixes, operation/artifact ids, catalogue, serde) plus the four upscale variants, both contracts, and artifact resolution | initialization slices (done) | **done** (archived `2026-09-11-add-models-1-upscale`) |
| 2 | `add-models-2-enhancements` | Denoise, Sharpen, Colorization, Light Adjustment, Color Balance - 13 variants, all one artifact each - plus the `Strength` (0-3) and `Bias` (-1..1) parameter types | 1 | **this change** |

With this slice the series is complete.

Three things sit downstream of this series and are deliberately **not** slices of it - each is its own
feature and gets planned as its own change when it is reached: Face Recovery and Detection (a parameter
that is a set of detected faces rather than a scalar, and a dependency on Detection to produce them),
downloading artifacts from HuggingFace, and inference itself. They are named here, as they were in
slice 1, so that whoever archives this change knows they were left open on purpose rather than missed.

Slice 1 took the hardest family first on purpose - upscale is the only one that puts a parameter in the
model's identity, the only one resolving to several artifacts, and the only one with two contracts.
That bet paid: everything this slice needs already exists, and the one change it makes to slice 1's
public surface is a rename.

## Crate split

**`opai` only.** No `gui` or `cli` work, for the same reason as slice 1: these are domain types both
front ends need - the GUI to populate its choosers and sliders, the CLI to parse and validate flags -
so by the layering test they belong in the core library, and neither front end consumes them until the
inference API exists. There is no user-visible surface in this slice and therefore no frontend
component, i18n key or Tauri command to add.

## Dependencies

**`rust-sak`: nothing applies.** Checked before designing, as in slice 1. This slice writes no
infrastructure - no hashing, HTTP, download, extraction, filesystem, caching, telemetry or hardware
probe - so no `rust-sak` module is relevant and no feature is added to the existing
`["crypto", "fetch", "fs", "sysinfo"]` set. This is a gap in neither direction: model naming is
application domain vocabulary, specific to this app, and does not belong in a general-purpose crate.
Nothing here is a candidate for extending `rust-sak`.

**No new third-party crates.** `serde` (with `derive`) and `thiserror` are already direct `opai`
dependencies and are all this needs. No alternative was weighed because nothing new is being chosen.

## Decisions

### `ConvPrecision` becomes `FloatPrecision` and is shared by all sixteen FP32/FP16 variants

All thirteen variants added here are published at FP32 and FP16 and nothing else - the exact domain
slice 1 modelled as `ConvPrecision` for the three convolutional upscalers. Handing them the wide
`Precision` instead would put `Int8` back in reach and make `dn_stockholm_int8` nameable, which is the
one invariant the whole precision design exists to hold.

The type is therefore renamed to `FloatPrecision`, a name that says what the domain is - published as
floating point, at either width - rather than what the three original users happened to be. Sixteen
variants now share it: Tokyo, Kyoto, Saitama and the thirteen here.

This is a rename of a Rust type, nothing more. The serde spelling lives on the *variants* (`"fp32"`,
`"fp16"`) and is untouched, so no persisted setting, artifact name or download URL changes. The edit
reaches `precision.rs`, `upscale/mod.rs`, `catalogue.rs`, `models/mod.rs`, `lib.rs` and
`tests/public_api.rs`, and is mechanical in all of them.

*Alternative considered:* leave `ConvPrecision` named as it is and use it anyway. Rejected - a reader
meeting `ColorizationVariant::Delhi(ConvPrecision)` has to already know the name is vestigial.
*Alternative considered:* a second, identical `EnhancementPrecision`. Rejected - two types for one
domain fact, each with its own `ALL` and its own widening impl, and a seventh family later has to
guess which.

### One operation type per family, not one generic enhancement type

The thirteen are uniform enough that a single `Enhancement { family, variant, amount: Option<_> }`
backed by one thirteen-row table would be materially less code. It is still the wrong shape, for
reasons that outlast the saving:

- The family becomes data instead of a type, so the inference pipeline's dispatch becomes a runtime
  match on a tag rather than an exhaustive one the compiler checks - and a family added later can fail
  to be handled. Slice 1's central move was making "operation type not supported" unrepresentable.
- Nothing would stop a denoise codename being paired with the sharpen family. The pairing is currently
  guaranteed by construction.
- Colorization would carry an `Option` that is structurally always `None`, which is the reference's
  "implements neither optional interface" expressed a second way.
- A family that later diverges - a second artifact, a third precision, a second parameter - would have
  to either widen the shared shape for everyone or be lifted out. Upscale is the proof that families do
  diverge.

So there are five public types - `Denoise`, `Sharpen`, `LightAdjustment`, `ColorBalance`,
`Colorization` - each with its own variant enum carrying `FloatPrecision`, mirroring `Upscale`:

```
pub enum DenoiseVariant { Stockholm(FloatPrecision), Gothenburg(FloatPrecision), Malmo(FloatPrecision) }
pub struct Denoise { variant: DenoiseVariant, strength: Strength }

pub struct Colorization { variant: ColorizationVariant }   // no parameter field at all
```

What they share is not a type but a table and a helper: each variant enum resolves to a
`SimpleVariant { codename, label }` row, and one internal function composes the single
`ArtifactId` from `(Family, codename, None, Precision)`. The repetition that remains is five small
enums and five short impls - the part that should be per-family, since it is exactly the part that
diverges.

### The parameter is outside the identity and inside the cache tag

This is upscale's Osaka rule applied to four more families, and it is what the reference already does:
its `Op.Id()` omits the intensity and its `CacheKey()` folds it in, so the registry reuses one session
across every value a user drags through while the cache still tells two runs apart.

Here it is structural rather than conventional. `PartialEq` and `Hash` are hand-written over the
variant alone, in the same shape and for the same reason as `Upscale`'s, and `cache_tag` is a total
method every family must answer - so a family that acquires a parameter cannot forget to fold it in.
In the reference that fold is an optional interface returning `""` for "nothing", which a new family
can silently fail to implement.

`Colorization` needs no hand-written `PartialEq` at all: it has only the field its identity is made of,
so the derive is correct. Writing one anyway would suggest there was something to exclude.

### Two named parameter types, not one shared `Intensity`

The reference calls both values "intensity" and passes both under the `"intensity"` map key, but they
are not one parameter: 0-3 centred on 1 means "how much of this effect", and -1..1 centred on 0 means
"in which direction, how far". A single type would have to carry its bounds per family, which is
exactly the drift the catalogue exists to prevent - and its neutral value would depend on who was
holding it.

`Strength` (0.0-3.0, neutral 1.0) and `Bias` (-1.0-1.0, neutral 0.0) are therefore separate, and each
follows `Scale`'s established pattern exactly: `MIN`/`MAX`/`NAME` constants that the catalogue and the
constructor both read, a rejecting `new` returning the shared `RangeError`, a clamping `clamped`, and
quantization on acceptance.

The names are developer-facing. The GUI's visible label stays whatever its i18n key says - the
reference shows "Intensity" for both - so no user-visible copy changes.

### Both parameters quantize to three decimals, stored as integers

The same decision `Scale` made, forced by the same constraint: the operation derives or hand-writes
`Hash` and `Eq` to key the registry and `f64` implements neither, and its `PartialEq` is unsound as an
identity anyway (`NaN != NaN`, `-0.0 == 0.0` with different bits). `Strength` stores `0..=3000` in a
`u16`; `Bias` stores `-1000..=1000` in an `i16`.

Three decimals is finer than anything the reference's interface can produce - its slider steps in whole
percent and its text field parses an integer percent - so no reachable input is rounded. It also closes
the reference's `"i=%.3g"` cache collision the same way `Scale` closed the `%.4g` one: two values that
would have shared a key become one value before anything is rendered.

### The cache tag extends slice 1's format rather than inventing one

`Upscale` reports `up-kyoto-fp32-s2`. The five families follow it: `{prefix}-{codename}-{precision}`,
plus `-t<value>` for a strength and `-b<value>` for a bias, and nothing more for colorization -
`dn-stockholm-fp32-t0.5`, `cb-rio-fp16-b-0.5`, `cl-delhi-fp32`.

Hyphen-separated for the reason slice 1 gave: artifact names are underscore-separated, so no cache tag
can be mistaken for or collide with a published file name. Distinct letters for the two parameters
rather than the reference's shared `i=`, so a tag says which parameter it carries. Written deliberately
and pinned by tests rather than derived from the serde output, so a field rename cannot invalidate
every cached image.

This is a clean break from the reference's cache keys, as slice 1 already was - there is no requirement
to read a cache written by the Go application.

### The catalogue reports an empty parameter list for colorization

`FamilyEntry.parameters` is already a `Vec`, so colorization reports an empty one and needs no change
to the type. That is better than omitting the family: a front end that has to infer "colorization
exists but has no control" from its absence is back to knowing something the library did not tell it.

The five entries are built from the same variant rows and the same `MIN`/`MAX`/`NAME` constants the
constructors enforce, so a control cannot offer a value the library will refuse. The existing
`every_listed_variant_is_one_that_can_actually_be_built` test generalizes to all six families and is
what keeps that true rather than merely intended.

Two things the reference's frontend registry also holds are deliberately **not** added: a default
variant per family, and the HD/SD "quality tier" naming that maps onto FP32/FP16. Both are real
duplication worth removing later, but both would change `FamilyEntry`'s shape for all six families and
neither is needed to name a model. They are noted for whichever change first wires a front end up.

## Ownership and lifetimes

Unchanged from slice 1, and worth restating because it is the property the registry design will build
on. Nothing here owns a native resource: every operation is a plain `Copy` value describing *what* to
run, with no `Arc`, no handle, no `Drop` and no reference to a loaded session. The description a front
end holds and serializes must outlive, and stay independent of, any session built from it. The
use-after-free hazard the project conventions call out is a property of sessions, which do not exist
yet.

## Platforms and execution providers

Platform-independent and provider-independent by construction: no artifact name, label or parameter
bound varies by operating system, architecture or execution provider. Nothing needs per-platform
validation and CI's existing matrix covers it. The provider-specific material in the reference's
variants is its `EPProfile` tuning, which is out of scope here and arrives with sessions - at which
point per-provider validation becomes real and will have to be stated.

## Risks / Trade-offs

- **An id or label transcribed wrong names an artifact that does not exist, or shows a user the wrong
  name** → Every one of the thirteen ids was read from `models/ids_test.go`, which pins them as
  literals precisely because they key the on-disk cache and the downloaded artifacts; the labels were
  read from each variant's own literal. The three that differ from this project's context prose -
  `Petersburg` not "St. Petersburg", `Malmo` not "Malmö", `São Paulo` with its accent - are called out
  in the proposal so the difference reads as verified rather than as a typo. Tests assert the full
  expected set per family.
- **Bounding a parameter the reference never bounded could refuse something a user could previously
  do** → Only through a path that bypasses the interface, since the React components already clamp to
  these exact bounds. The clamping constructor exists so the GUI path, whose input is already bounded,
  never surfaces an error it cannot act on.
- **The rename touches slice 1's public API** → Mechanical, compiler-verified, and confined to a Rust
  type name; the serialized spelling and every artifact name are untouched. `tests/public_api.rs`
  exercises the renamed type the way a consumer reaches it, so an incomplete rename fails there rather
  than at a call site months later.
- **Five near-identical family modules will read as copy-paste** → Accepted, and the shape is chosen so
  the duplicated part is small: the variant rows are a table, the artifact composition is one shared
  helper, and what repeats is the per-family enum and its impls. Collapsing that last part is what the
  generic alternative above would buy, at the cost of the guarantees listed there.
