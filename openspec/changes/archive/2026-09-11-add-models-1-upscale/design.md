## Context

See `proposal.md` - Why. What matters for the approach:

`opai` today installs the ONNX Runtime and GPU libraries and reports the execution providers a
machine supports. It has no model vocabulary at all. The existing `deps` layer is the only nearby
precedent, and one detail of it is worth carrying: `deps::release::Dependency` already holds
`sources: Vec<Source>` with the comment *"One for a release archive; a model splits into several"*.
Multi-artifact models were anticipated there; this slice is where they acquire names.

The reference implementation (`../open-photo-ai/models/upscale/`) is the behavioural source of truth
for every id, filename and label. It is not the structural model - most of its shape exists to work
around Go's type system, and this design deliberately departs from it where that is the case.

## Goals / Non-Goals

**Goals:**

- A value that names one upscale model, carries its parameters, and cannot be constructed in a state
  the published artifacts do not cover.
- Resolution from such a value to the artifacts it needs - the piece the inference API will call.
- A vocabulary (precision, family prefixes, artifact names, the cache tag, the catalogue) that slice 2 extends
  rather than reworks.

**Non-Goals:**

- Any inference, tiling, session building or execution-provider tuning. The `EPProfile` bodies in the
  Go variants - hundreds of lines of measured CoreML/CUDA/TensorRT findings - are **not** ported here
  and are not lost: they belong with the sessions those settings configure.
- Downloading model artifacts. This slice names them; fetching them is a separate change.
- The model registry, memory budgets and leases. No session exists yet, so there is no lifetime to
  own - see *Ownership and lifetimes* below.
- Face Recovery and Detection. Their own change later, planned separately.

## Slices

This series is the model scaffold for the six families being scaffolded now, and nothing else. It was
estimated at roughly 24 tasks, over the 15 limit, so it is split in two.

| # | Slice | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-models-1-upscale` | Shared model vocabulary (precision, family prefixes, operation/artifact ids, catalogue, serde) plus the four upscale variants, both contracts, and artifact resolution | initialization slices (done) | **this change** |
| 2 | `add-models-2-enhancements` | Denoise, Sharpen, Colorization, Light Adjustment, Color Balance - 13 variants, all one artifact each - plus the `Strength` (0-3) and `Bias` (-1..1) parameter types | 1 | pending |

Three things sit downstream of this series and are deliberately **not** slices of it - each is its own
feature and gets planned as its own change when it is reached, not carried as a pending row here:
Face Recovery and Detection (a parameter that is a set of detected faces rather than a scalar, and a
dependency on Detection to produce them), downloading artifacts from HuggingFace, and inference
itself. They are named here only so that whoever archives this change knows they were left open on
purpose.

Slice 1 is deliberately the hardest family rather than the easiest. Upscale is the only family that
puts a parameter in the model's identity, the only one resolving to several artifacts, and the only
one with two contracts. Designing the vocabulary against the thirteen uniform models in slice 2 and
meeting Osaka afterwards would mean reworking it.

## Crate split

**`opai` only.** No `gui` or `cli` work in this slice. These are domain types both front ends need -
the GUI to populate its chooser and sliders, the CLI to parse and validate flags - so by the layering
test they belong in the core library. Neither front end consumes them until the inference API exists,
so there is nothing to wire up yet.

## Dependencies

**`rust-sak`: nothing applies.** Checked before designing. This slice writes no infrastructure - no
hashing, HTTP, download, extraction, filesystem, caching, telemetry or hardware-probe code - so no
`rust-sak` module is relevant and no feature is added to the existing
`["crypto", "fetch", "fs", "sysinfo"]` set. This is a gap in neither direction: model naming is
application domain vocabulary, specific to this app, and does not belong in a general-purpose crate.
`fetch` and `crypto` become relevant when artifacts are actually downloaded, in a later change.

**New third-party crates: `serde` (with `derive`) only.** Already present transitively through Tauri
on the `gui` side; this adds it as a direct `opai` dependency. No alternative was weighed because the
requirement is interoperating with Tauri's `invoke` boundary, which is serde-based.

## Decisions

### Precision is split by what each variant publishes, not validated at runtime

Osaka is published as FP16 and INT8; the other three as FP32 and FP16. Rather than one `Precision`
enum plus a per-variant check, the precision travels **inside** the variant enum, in the domain that
variant actually admits:

```
pub struct Upscale { variant: UpscaleVariant, scale: Scale }

pub enum UpscaleVariant {
    Tokyo(ConvPrecision), Kyoto(ConvPrecision), Saitama(ConvPrecision),
    Osaka(OsakaPrecision),
}
```

`Osaka` paired with FP32 is then not a runtime error - it does not compile. Scale sits outside the
variant because all four take the same range; precision sits inside because its domain does not.

A single `Precision { Fp32, Fp16, Int8 }` still exists as the common currency that renders into ids
and filenames, with infallible widening from each per-contract enum. This keeps the catalogue and the
id formatter from needing to know which contract they were handed.

*Alternative considered:* one enum plus a `Result` from every constructor. Rejected - it pushes an
error onto every caller for a condition that is a fact about the published files, not about input.
*Alternative considered:* a per-variant builder with no `.fp32()` method on Osaka. Equivalent
guarantee, better ergonomics, materially more code; revisit if the enum proves awkward in slice 2.

### The operation is its own identity; there is no operation id

Go gives every operation a string id and keys four separate things on it: the model registry, the
image cache, the log lines, and the progress events the frontend receives. That is a workaround for a
Go limitation rather than a design - a sum type cannot derive equality or hashing, and a struct
holding an interface is unsafe as a map key, so a string stands in for the value. Rust derives
`PartialEq`, `Eq` and `Hash` on the operation directly, and the registry keys on `(Operation,
Provider)` with no string in between.

Dropping it is worth more than the code it saves, because flattening a value into a string produces a
consumer that parses it back. The reference's frontend recovers the family with `opId.slice(0, 2)`
and the precision with `id.split('_')[2]`; the latter is positionally correct only for a
three-segment id, and is kept away from `up_kyoto_2.5x_fp32` - where index 2 is the scale, not the
precision - by a `startsWith('fr_')` guard standing in front of it. The same frontend already carries
a structured options bag and reads the scale from *that*, so the information exists twice in two
forms. Carrying structured data across the boundary removes the parsing and the trap together.

Each of the four uses gets what it actually needed:

| Use | What it gets |
|---|---|
| Model registry key | the operation value, via derived `Hash` + `Eq` |
| Image cache key | a cache tag - see below, the one string that survives |
| Log lines and UI | the display label, `Kyoto 4x (FP16)` |
| Dispatch | an exhaustive `match`; Go's "operation type not supported" is unrepresentable |

Nothing is lost by the drop: this is a clean break, with no requirement to read settings persisted by
the Go application.

*Artifact ids stay*, unchanged and non-negotiable: `up_kyoto_4x_fp32` names a file published on
HuggingFace. This remains a newtype rather than a `String`, which is the same move the last two
initialization changes made (`refactor-init-roles-and-errors`, `0189c0a`) - but the look-alike-pair
hazard it was guarding against is mostly gone, because there is no longer a second string that
resembles it.

### The image cache tag is the one string that survives

The image cache persists to disk and outlives the process, so value equality alone cannot key it. Each
operation reports a cache tag: equal for operations that produce the same image, distinct for
operations that do not - including for a parameter that does not distinguish the operation itself,
which is Osaka's scale.

It is written deliberately and pinned by tests rather than derived from the serde output. Deriving it
would tie the on-disk cache to a field-naming decision, so a rename would silently invalidate every
cached image. Nothing else in the system keys on it, which is what keeps it free to change under its
own tests.

### An artifact id is a stem, not a filename

Resolution yields `up_osaka_fp16`, not `up_osaka_fp16.onnx`. One artifact is often several files: a
large model carries its weights in a sibling `.onnx.data`, and the reference implementation discovers
those by prefix-matching the remote file listing rather than predicting them. Naming a concrete file
here would either hardcode a guess about which artifacts have external weights, or duplicate a
decision that belongs to whichever change adds downloading. The stem is what both agree on.

### Scale is quantized on acceptance rather than formatted on rendering

Go renders the scale into the id with `%.4g`, and the id is the image cache key - so `1.66661x` and
`1.66669x` produce one string, collide, and the second request is served the first's image.

`Scale` stores a value quantized to three decimal places, chosen because for the range 1-8 that is
exactly what four significant figures expresses - so every artifact name and cache tag the reference
produces is reproduced character for character. The collision becomes unreachable rather than tested
for: two inputs that would have shared a string are now the same `Scale`, hence the same operation.

`Scale` stores that quantized value as an **integer** in milli-units (1000..=8000), not as an `f64`.
This is forced rather than stylistic: the operation derives `Hash` and `Eq` so it can key the
registry, `f64` implements neither, and its `PartialEq` is unsound as an identity anyway - `NaN`
compares unequal to itself and `-0.0 == 0.0` while the two have different bit patterns. An integer
makes the comparison exact, the hashing sound, and the rendering lossless, all from one decision.

Both `Scale::new(f64) -> Result<Scale, RangeError>` and `Scale::clamped(f64) -> Scale` are provided.
The GUI slider is already bounded and wants the infallible one; a CLI flag is not and wants the
error. Both guarantee an accepted value is in range, so nothing downstream re-checks.

### Per-run parameters are typed and their cache contribution is total

Go passes them as `map[string]any` keyed by `"scale"` / `"intensity"`, and the reader substitutes a
default on a missing key or wrong type - so a typo is a wrong image rather than an error. And the
cache-key contribution is an *optional* interface returning `""` for "nothing", so a family that
carries a per-run parameter and forgets to implement it collides silently.

Both become methods on the family type: the parameters are a typed struct, and the cache-key
contribution is a total function whose "nothing to add" case is written down. Exhaustive matching
means a variant added later cannot fail to answer.

### The catalogue is data the front ends read, not literals they repeat

Today `min={0} max={300}` is a literal in `OptionsDenoise/index.tsx` and `-100/100` is a default in a
shared component's props, with nothing connecting either to the model code. The catalogue publishes
the variants, their labels, their admitted precisions and each parameter's bounds, and the same
bounds constant backs both the catalogue entry and the `Scale` constructor - so a control cannot
offer a value the library will refuse.

The catalogue is a static description, built from the same variant data that drives resolution rather
than written out a second time beside it.

### Variant data is a table, not a type per variant

Go gives each variant its own package containing a registration shim and a `Variant` literal. Most of
that file is the `EPProfile` comment; the data itself is four fields. Here each variant is a row -
codename, label, precisions, and either a scale-bucket table or a graph set. Adding a variant is a
row, and the exhaustiveness of the enum is what makes the row mandatory.

The two contracts stay visibly separate rather than becoming optional fields on one shape: the
convolutional one maps a requested scale onto native passes, the diffusion one holds three
role-addressed graphs with the scale outside the identity.

### Osaka's graphs are addressed by role

Its three graphs - VAE encoder, diffusion transformer, VAE decoder - are bound by name, not by
position in a list. Reordering a list silently exchanges the encoder and the decoder: it compiles,
runs, and returns a wrong image. The two VAE halves pin FP16 in their own row rather than following
the operation, because no INT8 build of them exists; an INT8 operation that let them follow would
resolve to `up_osaka_vae_encoder_int8`, which is not published.

## Ownership and lifetimes

Nothing in this slice owns a native resource. An operation is a plain value - `Copy`-able or cheaply
clonable, no `Arc`, no handle, no `Drop` - describing *what* to run, with no reference to a loaded
session. That separation is the point, and it is what a later registry design will build on:
the description a front end holds and serializes must outlive, and be independent of, any session
built from it. The use-after-free hazard the conventions call out is a property of sessions, which do
not exist yet.

## Platforms and execution providers

Platform-independent and provider-independent by construction: no artifact name, id, label or
parameter bound varies by operating system, architecture or execution provider. Nothing here needs
per-platform validation, and CI's existing matrix covers it. The provider-specific material in the Go
variants is the `EPProfile` tuning, which is explicitly out of scope and arrives with sessions in
a later change - at which point per-provider validation becomes real and will have to be stated.

## Risks / Trade-offs

- **An id or filename transcribed wrong names an artifact that does not exist** → Every id in the
  spec's scenarios was read from the reference's own tests (`models/ids_test.go`) and variant tables,
  not inferred from the naming pattern. Tests assert the full expected set per variant, and the
  failure surfaces at download time rather than silently.
- **The per-contract precision enums could prove awkward when slice 2 adds thirteen variants that all
  take the same two precisions** → Those families need no split at all; they take the common
  `Precision` directly. The split exists only where a variant's domain genuinely differs, which so far
  is Osaka alone. If a second such case appears and the pattern does not scale, the builder
  alternative above is the fallback and costs no spec change.
- **Quantizing the scale is a behavioural divergence from the reference** → Only for inputs finer than
  three decimals, which the GUI cannot produce (free-text field, clamped to 1-8) and which in the
  reference are already silently collapsed - into a cache collision rather than a quantized value. The
  divergence replaces a wrong image with a slightly-rounded scale, and is recorded in the proposal.
- **The serde representation becomes a compatibility surface for persisted settings** → Deliberate, and
  the reason serde lands now rather than after the shapes are consumed; retrofitting it would churn a
  public API two front ends already depend on. Variants are tagged by name rather than by position, so
  adding a family or a variant later does not invalidate anything already persisted.
- **Slice 2 could discover the vocabulary does not fit** → The reason upscale goes first. Slice 2's
  families are strictly simpler on every axis this design covers - one artifact, no parameter in the
  identity, one precision domain - so the risk is over-generality, not under-generality.
